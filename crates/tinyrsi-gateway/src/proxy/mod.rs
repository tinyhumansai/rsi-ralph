//! The OpenAI-compatible HTTP surface and the forwarding pipeline.
//!
//! Every call goes through the same steps: prepare (validate, pin, bound),
//! reserve against both ledgers, forward to the upstream, read the actual
//! usage (scanning the stream as it passes through), settle, and meter.
//!
//! Routes:
//!
//! - `POST /v1/chat/completions`, `POST /v1/embeddings`
//! - the same under `/__tag/{phase}/{seat}/v1/...`, for clients that cannot
//!   set `x-tinyrsi-*` headers
//! - `GET /v1/models` (also under the tag prefix) — the allowlist
//! - `GET /status` — both ledgers

mod models;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::Notify;

use crate::calls::CallTally;
use crate::error::Error;
use crate::ledger::{Budgets, LedgerState, Reservation};
use crate::meter::{Meter, MeterRow, Tags};
use crate::price::PriceTable;
use crate::request::{Endpoint, Limits, Prepared, prepare};
use crate::usage::{SseUsage, Usage};

/// Header naming the caller's loop phase.
pub(crate) const PHASE_HEADER: &str = "x-tinyrsi-phase";
/// Header naming the caller's seat within the phase.
pub(crate) const SEAT_HEADER: &str = "x-tinyrsi-seat";
/// Header naming the candidate a call is for.
pub(crate) const CANDIDATE_HEADER: &str = "x-tinyrsi-candidate";

/// Upstream failure text kept in the meter, in characters.
const FAILURE_EXCERPT_CHARS: usize = 500;

/// What the proxy needs from the gateway configuration.
pub(crate) struct ProxyConfig {
    /// Upstream base URL, such as `https://openrouter.ai/api/v1`.
    pub(crate) upstream: String,
    /// The real provider key; clients never see it.
    pub(crate) api_key: String,
    /// The model allowlist and price ceilings.
    pub(crate) prices: PriceTable,
    /// Completion-length limits.
    pub(crate) limits: Limits,
    /// When set, every forwarded request body is written here as
    /// `<seq>-<phase>.json`.
    pub(crate) capture_dir: Option<PathBuf>,
}

impl std::fmt::Debug for ProxyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyConfig")
            .field("upstream", &self.upstream)
            .field("api_key", &"[redacted]")
            .field("prices", &self.prices)
            .field("limits", &self.limits)
            .field("capture_dir", &self.capture_dir)
            .finish()
    }
}

/// A running gateway's shared state.
#[derive(Debug)]
pub(crate) struct Proxy {
    config: ProxyConfig,
    budgets: Mutex<Budgets>,
    meter: Meter,
    calls: CallTally,
    client: reqwest::Client,
    sequence: AtomicU64,
    active: AtomicUsize,
    idle: Notify,
}

impl Proxy {
    /// Builds a proxy over `budgets`, metering to `meter`.
    pub(crate) fn new(
        config: ProxyConfig,
        budgets: Budgets,
        meter: Meter,
        client: reqwest::Client,
    ) -> Arc<Self> {
        Arc::new(Self {
            config,
            budgets: Mutex::new(budgets),
            meter,
            calls: CallTally::default(),
            client,
            sequence: AtomicU64::new(0),
            active: AtomicUsize::new(0),
            idle: Notify::new(),
        })
    }

    /// The axum router serving every gateway route.
    pub(crate) fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/v1/chat/completions", post(chat))
            .route("/v1/embeddings", post(embeddings))
            .route("/v1/models", get(models))
            .route(
                "/__tag/{phase}/{seat}/v1/chat/completions",
                post(tagged_chat),
            )
            .route(
                "/__tag/{phase}/{seat}/v1/embeddings",
                post(tagged_embeddings),
            )
            .route("/__tag/{phase}/{seat}/v1/models", get(models))
            .route("/status", get(status))
            // Long agent sessions resend their whole history every call.
            .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024))
            .with_state(self)
    }

    /// Copies of the run and total ledger states, in that order.
    pub(crate) fn states(&self) -> (LedgerState, LedgerState) {
        let budgets = self.budgets.lock().unwrap_or_else(PoisonError::into_inner);
        let (run, total) = budgets.states();
        (run.clone(), total.clone())
    }

    /// The tally of calls forwarded upstream.
    pub(crate) fn calls(&self) -> &CallTally {
        &self.calls
    }

    /// Waits until no call is between reserving and settling, or `limit`
    /// elapses. Returns whether the proxy went idle.
    pub(crate) async fn drain(&self, limit: Duration) -> bool {
        tokio::time::timeout(limit, async {
            loop {
                let notified = self.idle.notified();
                if self.active.load(Ordering::Acquire) == 0 {
                    return;
                }
                notified.await;
            }
        })
        .await
        .is_ok()
    }

    fn reserve(&self, amount: f64) -> Result<Reservation, Error> {
        self.budgets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .reserve(amount)
    }

    fn settle(&self, reservation: Reservation, cost: Option<f64>) -> Option<String> {
        self.budgets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .settle(reservation, cost)
            .err()
            .map(|error| error.to_string())
    }

    fn capture(&self, tags: &Tags, body: &Value) {
        let Some(dir) = &self.config.capture_dir else {
            return;
        };
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed);
        let record = json!({ "tags": tags, "body": body });
        let path = dir.join(capture_name(seq, &tags.phase));
        let written =
            std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, record.to_string()));
        if let Err(error) = written {
            tracing::warn!(path = %path.display(), %error, "tinyrsi-gateway: capture failed");
        }
    }

    fn record(&self, row: &MeterRow) {
        // A meter write failure must not fail the call the ledger already
        // charged; it is reported through `tracing` instead.
        if let Err(error) = self.meter.record(row) {
            tracing::warn!(%error, "tinyrsi-gateway: meter write failed");
        }
    }

    /// Removes the provider key from text that may echo it back.
    fn redact(&self, text: &str) -> String {
        redact(text, &self.config.api_key)
    }

    /// A buffered upstream body with the provider key removed, should the
    /// upstream ever echo it (an auth error quoting the credential). Bodies
    /// that do not contain it pass through untouched.
    fn redact_body(&self, body: Bytes) -> Bytes {
        let key = &self.config.api_key;
        match std::str::from_utf8(&body) {
            Ok(text) if !key.is_empty() && text.contains(key.as_str()) => {
                Bytes::from(redact(text, key))
            }
            _ => body,
        }
    }
}

/// Counts one call as active until dropped, so shutdown can wait for it.
struct InFlight(Arc<Proxy>);

impl InFlight {
    fn new(proxy: &Arc<Proxy>) -> Self {
        proxy.active.fetch_add(1, Ordering::AcqRel);
        Self(Arc::clone(proxy))
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        if self.0.active.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.0.idle.notify_waiters();
        }
    }
}

/// The capture file name for call `seq` in `phase`.
///
/// The phase comes from the request path or a header, so anything but ASCII
/// letters, digits, `-` and `_` is replaced: a capture can never escape its
/// directory.
fn capture_name(seq: u64, phase: &str) -> String {
    let phase: String = if phase.is_empty() {
        "untagged".to_owned()
    } else {
        phase
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    format!("{seq:06}-{phase}.json")
}

/// `text` with every occurrence of `secret` replaced by `[redacted]`.
fn redact(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        text.to_owned()
    } else {
        text.replace(secret, "[redacted]")
    }
}

type Shared = State<Arc<Proxy>>;

async fn chat(State(proxy): Shared, headers: HeaderMap, body: Bytes) -> Response {
    detached(forward(proxy, Endpoint::Chat, header_tags(&headers), body)).await
}

async fn embeddings(State(proxy): Shared, headers: HeaderMap, body: Bytes) -> Response {
    detached(forward(
        proxy,
        Endpoint::Embeddings,
        header_tags(&headers),
        body,
    ))
    .await
}

async fn tagged_chat(
    State(proxy): Shared,
    Path((phase, seat)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    detached(forward(
        proxy,
        Endpoint::Chat,
        path_tags(&headers, phase, seat),
        body,
    ))
    .await
}

async fn tagged_embeddings(
    State(proxy): Shared,
    Path((phase, seat)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    detached(forward(
        proxy,
        Endpoint::Embeddings,
        path_tags(&headers, phase, seat),
        body,
    ))
    .await
}

async fn models(State(proxy): Shared) -> Response {
    axum::Json(models::listing(&proxy).await).into_response()
}

async fn status(State(proxy): Shared) -> Response {
    let (run, total) = proxy.states();
    axum::Json(json!({ "run": run, "total": total })).into_response()
}

/// Runs a forward on its own task and waits for it.
///
/// axum drops a handler's future when the client disconnects. A forward
/// dropped between reserving and settling would leave its reservation in
/// flight forever, so the upstream call and its settlement run detached and
/// finish even if nobody is left to read the response.
async fn detached(call: impl Future<Output = Response> + Send + 'static) -> Response {
    tokio::spawn(call).await.unwrap_or_else(|error| {
        error_response(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string())
    })
}

fn header_tags(headers: &HeaderMap) -> Tags {
    let get = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    };
    Tags {
        phase: get(PHASE_HEADER),
        seat: get(SEAT_HEADER),
        candidate: get(CANDIDATE_HEADER),
    }
}

fn path_tags(headers: &HeaderMap, phase: String, seat: String) -> Tags {
    Tags {
        phase,
        seat,
        ..header_tags(headers)
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn error_status(error: &Error) -> StatusCode {
    match error {
        Error::BudgetExceeded { .. } | Error::PriceBoundBreached => StatusCode::PAYMENT_REQUIRED,
        Error::UnknownModel(_) => StatusCode::FORBIDDEN,
        Error::Upstream(_) => StatusCode::BAD_GATEWAY,
        Error::BadRequest(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        axum::Json(json!({ "error": { "message": message, "code": status.as_u16() } })),
    )
        .into_response()
}

/// The outcome of one call, before it is metered.
struct Outcome {
    status: u16,
    stream: bool,
    reserved: f64,
    usage: Usage,
    cost: Option<f64>,
    error: Option<String>,
}

async fn forward(proxy: Arc<Proxy>, endpoint: Endpoint, tags: Tags, body: Bytes) -> Response {
    let in_flight = InFlight::new(&proxy);
    let started = Instant::now();
    let capture_tags = tags.clone();
    let row = move |model: &str, outcome: Outcome| MeterRow {
        at_ms: now_ms(),
        tags: tags.clone(),
        model: model.to_owned(),
        endpoint: endpoint.path().to_owned(),
        status: outcome.status,
        stream: outcome.stream,
        reserved_usd: outcome.reserved,
        cost_usd: outcome.cost,
        prompt_tokens: outcome.usage.prompt_tokens,
        completion_tokens: outcome.usage.completion_tokens,
        wall_ms: started.elapsed().as_millis(),
        error: outcome.error,
    };
    let refuse = |model: &str, error: &Error| {
        let status = error_status(error);
        proxy.record(&row(
            model,
            Outcome {
                status: status.as_u16(),
                stream: false,
                reserved: 0.0,
                usage: Usage::default(),
                cost: None,
                error: Some(error.to_string()),
            },
        ));
        error_response(status, &error.to_string())
    };
    let parsed = serde_json::from_slice::<Value>(&body)
        .map_err(|error| Error::BadRequest(format!("invalid JSON: {error}")))
        .and_then(|value| prepare(endpoint, value, &proxy.config.prices, proxy.config.limits));
    let prepared = match parsed {
        Ok(prepared) => prepared,
        Err(error) => return refuse("", &error),
    };
    proxy.capture(&capture_tags, &prepared.body);
    let reservation = match proxy.reserve(prepared.reserve_usd) {
        Ok(reservation) => reservation,
        Err(error) => return refuse(&prepared.model, &error),
    };
    let url = format!(
        "{}/{}",
        proxy.config.upstream.trim_end_matches('/'),
        endpoint.path()
    );
    let call = proxy.calls.dispatched();
    let sent = proxy
        .client
        .post(url)
        .bearer_auth(&proxy.config.api_key)
        .json(&prepared.body)
        .send()
        .await;
    let response = match sent {
        Ok(response) => response,
        Err(error) => {
            let error = Error::Upstream(proxy.redact(&error.to_string()));
            proxy.calls.unreachable(call, &error.to_string());
            let settle_error = proxy.settle(reservation, None);
            proxy.record(&row(
                &prepared.model,
                Outcome {
                    status: StatusCode::BAD_GATEWAY.as_u16(),
                    stream: prepared.stream,
                    reserved: reservation.amount(),
                    usage: Usage::default(),
                    cost: None,
                    error: Some(settle_error.unwrap_or_else(|| error.to_string())),
                },
            ));
            return error_response(StatusCode::BAD_GATEWAY, &error.to_string());
        }
    };
    if prepared.stream && response.status().is_success() {
        return stream_through(
            in_flight,
            call,
            prepared,
            reservation,
            response,
            move |outcome| row(&outcome.0, outcome.1),
        );
    }
    let model = prepared.model.clone();
    let (response, outcome) = buffered(&proxy, call, &prepared, reservation, response).await;
    proxy.record(&row(&model, outcome));
    drop(in_flight);
    response
}

/// Reads a non-streaming (or failed) response whole, then settles it.
/// `call` is the call's position in the upstream tally.
async fn buffered(
    proxy: &Proxy,
    call: u64,
    prepared: &Prepared,
    reservation: Reservation,
    response: reqwest::Response,
) -> (Response, Outcome) {
    let status = response.status();
    // A body that breaks off is no answer, whatever the status said.
    let bytes = match response.bytes().await {
        Ok(bytes) => {
            let bytes = proxy.redact_body(bytes);
            proxy.calls.answered(call, status.as_u16(), &bytes);
            bytes
        }
        Err(error) => {
            proxy
                .calls
                .unreachable(call, &proxy.redact(&error.to_string()));
            Bytes::new()
        }
    };
    let usage = serde_json::from_slice::<Value>(&bytes)
        .map_or_else(|_| Usage::default(), |v| Usage::from_body(&v));
    let price = proxy.config.prices.get(&prepared.model).ok();
    let cost = if status.is_success() {
        price.and_then(|p| usage.cost_or_ceiling(p))
    } else if status.is_client_error() && status != StatusCode::REQUEST_TIMEOUT {
        // OpenRouter does not bill a request it rejects outright.
        Some(usage.cost.unwrap_or(0.0))
    } else {
        usage.cost
    };
    let settle_error = proxy.settle(reservation, cost);
    let failure = (!status.is_success()).then(|| {
        let excerpt: String = String::from_utf8_lossy(&bytes)
            .chars()
            .take(FAILURE_EXCERPT_CHARS)
            .collect();
        proxy.redact(&excerpt)
    });
    let outcome = Outcome {
        status: status.as_u16(),
        stream: false,
        reserved: reservation.amount(),
        usage,
        cost,
        error: settle_error.or(failure),
    };
    let response = Response::builder()
        .status(status.as_u16())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "response build failed")
        });
    (response, outcome)
}

fn stream_through<F>(
    in_flight: InFlight,
    call: u64,
    prepared: Prepared,
    reservation: Reservation,
    response: reqwest::Response,
    row: F,
) -> Response
where
    F: FnOnce((String, Outcome)) -> MeterRow + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    tokio::spawn(async move {
        let proxy = &in_flight.0;
        let mut scanner = SseUsage::default();
        let mut upstream = response.bytes_stream();
        let mut failure = None;
        while let Some(chunk) = upstream.next().await {
            match chunk {
                Ok(chunk) => {
                    scanner.feed(&chunk);
                    // Keep draining after the client goes away: the usage
                    // event arrives last and the call is billed regardless.
                    let _ = tx.send(Ok(chunk)).await;
                }
                Err(error) => {
                    let message = proxy.redact(&error.to_string());
                    let _ = tx.send(Err(std::io::Error::other(message.clone()))).await;
                    failure = Some(message);
                    break;
                }
            }
        }
        drop(tx);
        // Only now is it known: a stream that broke off was never answered.
        if let Some(message) = &failure {
            proxy.calls.unreachable(call, message);
        }
        let usage = scanner.finish();
        let cost = proxy
            .config
            .prices
            .get(&prepared.model)
            .ok()
            .and_then(|price| usage.cost_or_ceiling(price));
        let settle_error = proxy.settle(reservation, cost);
        proxy.record(&row((
            prepared.model.clone(),
            Outcome {
                status: 200,
                stream: true,
                reserved: reservation.amount(),
                usage,
                cost,
                error: settle_error.or(failure),
            },
        )));
        drop(in_flight);
    });
    let body = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(body))
        .unwrap_or_else(|_| {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "response build failed")
        })
}

#[cfg(test)]
mod test;
