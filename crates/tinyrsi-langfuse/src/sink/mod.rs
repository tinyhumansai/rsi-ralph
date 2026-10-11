//! A [`TraceSink`] that exports loop events to Langfuse over OTLP.
//!
//! Events become `OpenHuman` trace spans as they arrive (see `spans.rs`). When a
//! run finishes, its spans go through `OpenHuman`'s `otlp_requests`, which
//! chunks them under the transport's span and byte caps, and each request is
//! posted with `LangfuseClient::send_otlp` to Langfuse's OTLP trace endpoint,
//! the supported ingestion path.
//!
//! The client's transport needs a Tokio runtime. A dedicated worker thread
//! owns that runtime for the sink's whole life, so the sink can be used, and
//! dropped, from inside another runtime without nesting or shutting one down
//! on an async thread.
//!
//! Export is best-effort: the trace is observability, not evidence, so a
//! failed export never fails the loop. It is counted in
//! [`LangfuseTrace::failures`], the requests stay pending, and an explicit
//! [`LangfuseTrace::flush`] retries them and reports the error.
mod spans;

use crate::{Error, Result};
use openhuman_embed::observe::langfuse::{ExportBrand, LangfuseClient, otlp_requests};
use serde_json::Value;
use spans::SpanBuilder;
use std::sync::mpsc;
use tinyrsi_trace::{LoopEvent, TraceSink};

/// One OTLP request for the worker, and where to report how it went.
type Job = (Value, mpsc::Sender<Result<()>>);

/// Exports loop events to Langfuse as one trace per run.
pub struct LangfuseTrace {
    jobs: mpsc::Sender<Job>,
    endpoint: String,
    environment: String,
    builder: SpanBuilder,
    pending: Vec<Value>,
    failures: u64,
}

impl LangfuseTrace {
    /// Exports through `client`, tagging traces with the `default`
    /// environment.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidConfig`] when the worker thread or its runtime
    /// cannot be started.
    pub fn new(client: LangfuseClient) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::InvalidConfig)?;
        let endpoint = client.otlp_endpoint();
        let (jobs, inbox) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("tinyrsi-langfuse".into())
            .spawn(move || {
                // Ends when the sink, and so the sender, is dropped; the
                // runtime is dropped here, never on a caller's async thread.
                for (request, reply) in inbox {
                    let sent = runtime
                        .block_on(client.send_otlp(request))
                        .map(|_| ())
                        .map_err(|_| Error::ExportFailed);
                    let _ = reply.send(sent);
                }
            })
            .map_err(|_| Error::InvalidConfig)?;
        Ok(Self {
            jobs,
            endpoint,
            environment: "default".into(),
            builder: SpanBuilder::default(),
            pending: vec![],
            failures: 0,
        })
    }

    /// Exports to the Langfuse project named by the environment:
    /// `LANGFUSE_BASE_URL`, `LANGFUSE_PUBLIC_KEY` and `LANGFUSE_SECRET_KEY`, or
    /// the `TinyHumans` proxy (`TINYHUMANS_LANGFUSE_PROXY_URL` with
    /// `TINYHUMANS_AUTH_TOKEN`), exactly as `OpenHuman` reads them.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidConfig`] when a required variable is missing.
    pub fn from_env() -> Result<Self> {
        Self::new(LangfuseClient::from_env().map_err(|_| Error::InvalidConfig)?)
    }

    /// Tags exported traces with a Langfuse environment, such as `production`.
    #[must_use]
    pub fn with_environment(mut self, environment: impl Into<String>) -> Self {
        self.environment = environment.into();
        self
    }

    /// OTLP requests built but not yet delivered.
    #[must_use]
    pub fn pending(&self) -> &[Value] {
        &self.pending
    }

    /// How many automatic exports failed.
    #[must_use]
    pub fn failures(&self) -> u64 {
        self.failures
    }

    /// Sends every pending request, in order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ExportFailed`] when Langfuse cannot be reached or
    /// refuses a request. That request and the ones after it stay pending,
    /// so a later flush retries them.
    pub fn flush(&mut self) -> Result<()> {
        while let Some(request) = self.pending.first() {
            let (reply, outcome) = mpsc::channel();
            self.jobs
                .send((request.clone(), reply))
                .map_err(|_| Error::ExportFailed)?;
            outcome.recv().map_err(|_| Error::ExportFailed)??;
            self.pending.remove(0);
        }
        Ok(())
    }
}

impl TraceSink for LangfuseTrace {
    /// Maps the event; a finished run is turned into OTLP requests and
    /// flushed. Never fails: a failed flush is counted and its requests kept
    /// for a later retry.
    fn record(&mut self, event: LoopEvent) -> tinyrsi_trace::Result<()> {
        if let Some(spans) = self.builder.push(&event, spans::now_ms()) {
            let brand = ExportBrand {
                product: "tinyrsi",
                version: env!("CARGO_PKG_VERSION"),
            };
            self.pending
                .extend(otlp_requests(&spans, &self.environment, &brand));
            if self.flush().is_err() {
                self.failures += 1;
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for LangfuseTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LangfuseTrace")
            .field("endpoint", &self.endpoint)
            .field("environment", &self.environment)
            .field("pending", &self.pending.len())
            .field("failures", &self.failures)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test;
