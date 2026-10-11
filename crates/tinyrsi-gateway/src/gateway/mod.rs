//! The in-process gateway handle: start, address, spend, shutdown.
//!
//! [`Gateway::start`] is synchronous. It claims the total ledger, opens both
//! ledgers and the meter, binds the loopback listener on the caller's
//! thread (so every setup error comes straight back), then serves on a
//! dedicated background thread that owns its own Tokio runtime. A caller
//! with no async runtime can therefore use it, and a caller inside one can
//! too: the gateway's runtime is never created or dropped on a runtime
//! thread.
//!
//! [`Gateway::shutdown`] (or dropping the handle) stops accepting
//! connections, lets open ones finish, waits up to [`DRAIN_LIMIT`] for calls
//! still between reserving and settling, and releases the ledger lock.

mod types;

use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::ledger::{Budgets, Ledger};
use crate::lock::LedgerLock;
use crate::meter::Meter;
use crate::proxy::{Proxy, ProxyConfig};

pub use types::{DEFAULT_LISTEN, DEFAULT_UPSTREAM, GatewayConfig, Spend};

/// How long shutdown waits for in-flight calls to settle. A call still
/// running after this keeps its reservation in flight on disk, which the
/// next open of the ledger converts to an uncertain charge.
pub const DRAIN_LIMIT: Duration = Duration::from_secs(10);

/// A running gateway. Dropping it shuts it down.
#[derive(Debug)]
pub struct Gateway {
    addr: SocketAddr,
    proxy: Arc<Proxy>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<JoinHandle<Result<()>>>,
}

impl Gateway {
    /// Starts a gateway and returns once it is listening.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NonLoopback`] for a non-loopback listen address,
    /// [`Error::Locked`] if another gateway owns the total ledger,
    /// [`Error::InvalidLimit`], [`Error::LimitChanged`] or
    /// [`Error::LedgerFormat`] for a ledger problem, [`Error::Io`] if a run
    /// file cannot be created, [`Error::Bind`] if the address cannot be
    /// bound, [`Error::Upstream`] if the HTTP client cannot be built, and
    /// [`Error::Runtime`] or [`Error::ServerStopped`] if the background
    /// thread cannot start.
    pub fn start(config: GatewayConfig) -> Result<Self> {
        if !config.listen.ip().is_loopback() {
            return Err(Error::NonLoopback(config.listen));
        }
        let lock = LedgerLock::acquire(&config.total_ledger)?;
        let budgets = Budgets::new(
            Ledger::open(&config.run_dir.join("budget.json"), config.run_limit_usd)?,
            Ledger::open(&config.total_ledger, config.total_limit_usd)?,
        );
        let meter = Meter::open(&config.run_dir.join("meter.jsonl"))?;
        let bind = |source| Error::Bind {
            addr: config.listen,
            source,
        };
        let listener = std::net::TcpListener::bind(config.listen).map_err(bind)?;
        listener.set_nonblocking(true).map_err(bind)?;
        let addr = listener.local_addr().map_err(bind)?;
        let client = reqwest::Client::builder()
            .build()
            .map_err(|error| Error::Upstream(error.to_string()))?;
        let proxy = Proxy::new(
            ProxyConfig {
                capture_dir: config.capture.then(|| config.run_dir.join("captures")),
                upstream: config.upstream,
                api_key: config.api_key,
                prices: config.prices,
                limits: config.limits,
            },
            budgets,
            meter,
            client,
        );
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let (ready_tx, ready) = sync_channel(1);
        let serving = Arc::clone(&proxy);
        let thread = std::thread::Builder::new()
            .name("tinyrsi-gateway".to_owned())
            .spawn(move || {
                let result = serve(listener, serving, stopped, &ready_tx);
                // The lock outlives every task that could still charge.
                drop(lock);
                result
            })
            .map_err(Error::Runtime)?;
        let mut gateway = Self {
            addr,
            proxy,
            stop: Some(stop),
            thread: Some(thread),
        };
        wait_ready(&ready, &mut gateway)?;
        Ok(gateway)
    }

    /// The address the gateway is listening on.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The gateway's root URL, such as `http://127.0.0.1:41234`. The
    /// untagged OpenAI-compatible base is this plus `/v1`.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The OpenAI-compatible base URL that tags every call made through it
    /// with `phase` and `seat`: `<base_url>/__tag/<phase>/<seat>/v1`.
    ///
    /// Both values are percent-encoded, so any string is safe; an empty one
    /// produces a URL the gateway does not route.
    #[must_use]
    pub fn tagged_url(&self, phase: &str, seat: &str) -> String {
        format!(
            "{}/__tag/{}/{}/v1",
            self.base_url(),
            encode_segment(phase),
            encode_segment(seat)
        )
    }

    /// What the run and total ledgers have charged so far, read directly
    /// from the gateway's state.
    #[must_use]
    pub fn spend(&self) -> Spend {
        let (run, total) = self.proxy.states();
        Spend {
            run_usd: run.charged_usd(),
            total_usd: total.charged_usd(),
            run_limit_usd: run.limit_usd,
            total_limit_usd: total.limit_usd,
        }
    }

    /// Stops the gateway and waits for its thread to finish.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Serve`] if the server failed while running, or
    /// [`Error::ServerStopped`] if its thread panicked.
    pub fn shutdown(mut self) -> Result<()> {
        self.stop()
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(stop) = self.stop.take() {
            // The server may already have exited; nothing then waits.
            let _ = stop.send(());
        }
        match self.thread.take() {
            Some(thread) => thread.join().map_err(|_| Error::ServerStopped)?,
            None => Ok(()),
        }
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        // Drop cannot report; `shutdown` is the way to see a failure.
        let _ = self.stop();
    }
}

/// Waits for the background thread to report that it is serving.
fn wait_ready(ready: &Receiver<Result<()>>, gateway: &mut Gateway) -> Result<()> {
    // A closed channel means the thread died before reporting.
    let outcome = ready.recv().unwrap_or(Err(Error::ServerStopped));
    if outcome.is_err() {
        // The thread has already returned; reap it.
        let _ = gateway.stop();
    }
    outcome
}

/// Report a setup failure to the starting thread, which turns it into the
/// error `Gateway::start` returns.
fn refuse(ready: &std::sync::mpsc::SyncSender<Result<()>>, error: Error) {
    let _ = ready.send(Err(error));
}

/// The background thread's body: build a runtime, serve until stopped,
/// then drain calls still settling.
fn serve(
    listener: std::net::TcpListener,
    proxy: Arc<Proxy>,
    stopped: tokio::sync::oneshot::Receiver<()>,
    ready: &std::sync::mpsc::SyncSender<Result<()>>,
) -> Result<()> {
    let built = tokio::runtime::Builder::new_multi_thread()
        .thread_name("tinyrsi-gateway-worker")
        .enable_all()
        .build()
        .map_err(Error::Runtime);
    let runtime = match built {
        Ok(runtime) => runtime,
        Err(error) => {
            refuse(ready, error);
            return Ok(());
        }
    };
    runtime.block_on(async move {
        let addr = listener.local_addr().ok();
        let adopted = tokio::net::TcpListener::from_std(listener).map_err(|source| Error::Bind {
            addr: addr.unwrap_or(DEFAULT_LISTEN),
            source,
        });
        let listener = match adopted {
            Ok(listener) => listener,
            Err(error) => {
                refuse(ready, error);
                return Ok(());
            }
        };
        let _ = ready.send(Ok(()));
        let served = axum::serve(listener, Arc::clone(&proxy).router())
            .with_graceful_shutdown(async {
                // A dropped sender is a stop too.
                let _ = stopped.await;
            })
            .await
            .map_err(Error::Serve);
        if !proxy.drain(DRAIN_LIMIT).await {
            tracing::warn!("tinyrsi-gateway: calls still in flight at shutdown");
        }
        served
    })
}

/// Percent-encodes `text` as one URL path segment.
fn encode_segment(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            // Writing to a `String` cannot fail.
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
mod test;
