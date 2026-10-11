//! End to end: a real gateway in front of a mock upstream on loopback.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::{Value, json};
use tinyrsi_gateway::{Error, GLM_FLASH, Gateway, GatewayConfig, Refusal, UpstreamCalls};

const KEY: &str = "sk-or-test-SECRET-7f3a9c";

type Seen = Arc<Mutex<Vec<(String, Value)>>>;

async fn mock_chat(
    State(seen): State<Seen>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    seen.lock().unwrap().push((auth.clone(), body.clone()));
    let text = body["messages"][0]["content"].as_str().unwrap_or("");
    match text {
        "reject" => (StatusCode::BAD_REQUEST, Json(json!({"error": "bad"}))).into_response(),
        "crash" => (StatusCode::INTERNAL_SERVER_ERROR, "oops").into_response(),
        // OpenRouter's shape for a key over its spending limit.
        "limit" => (
            StatusCode::FORBIDDEN,
            Json(json!({"error": {"message": "Key limit exceeded (daily limit).", "code": 403}})),
        )
            .into_response(),
        // A misbehaving upstream that echoes the credential it was sent.
        "echo" => (
            StatusCode::UNAUTHORIZED,
            format!("invalid credential: {auth}"),
        )
            .into_response(),
        "overcost" => Json(json!({"choices": [], "usage": {"cost": 10.0}})).into_response(),
        "slow" => {
            tokio::time::sleep(Duration::from_millis(300)).await;
            Json(json!({"choices": [], "usage": {"cost": 0.003}})).into_response()
        }
        "tokens" => Json(
            json!({"choices": [], "usage": {"prompt_tokens": 1000, "completion_tokens": 1000}}),
        )
        .into_response(),
        _ if body["stream"] == json!(true) => {
            let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: {\"choices\":[],\"usage\":{\"cost\":0.001,\"prompt_tokens\":5,\"completion_tokens\":1}}\n\ndata: [DONE]\n\n";
            ([("content-type", "text/event-stream")], sse).into_response()
        }
        _ => Json(json!({"choices": [{"message": {"content": "ok"}}], "usage": {"cost": 0.002, "prompt_tokens": 5, "completion_tokens": 1}})).into_response(),
    }
}

async fn mock_embeddings(Json(_body): Json<Value>) -> Response {
    Json(json!({"data": [{"embedding": [0.0]}], "usage": {"cost": 0.00001, "prompt_tokens": 2}}))
        .into_response()
}

/// A mock upstream on its own thread and runtime, so a test blocking on
/// the gateway's shutdown never starves it.
struct Upstream {
    addr: SocketAddr,
    seen: Seen,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Upstream {
    fn start() -> Self {
        let seen: Seen = Arc::default();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new()
            .route("/v1/chat/completions", post(mock_chat))
            .route("/v1/embeddings", post(mock_embeddings))
            .with_state(seen.clone());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let thread = std::thread::spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    axum::serve(listener, app)
                        .with_graceful_shutdown(async {
                            let _ = stopped.await;
                        })
                        .await
                        .unwrap();
                });
        });
        Self {
            addr,
            seen,
            stop: Some(stop),
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }
}

impl Drop for Upstream {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Harness {
    // Field order is drop order: the gateway stops before its upstream.
    gateway: Gateway,
    upstream: Upstream,
    dir: tempfile::TempDir,
    client: reqwest::Client,
}

impl Harness {
    fn start(run_limit: f64, total_limit: f64) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let upstream = Upstream::start();
        let mut config = GatewayConfig::new(
            KEY,
            dir.path().join("run-1"),
            dir.path().join("ledger-total.json"),
            run_limit,
            total_limit,
        );
        config.upstream = upstream.url();
        config.capture = true;
        let gateway = Gateway::start(config).unwrap();
        Self {
            gateway,
            upstream,
            dir,
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
        }
    }

    fn base(&self) -> String {
        self.gateway.base_url()
    }

    async fn chat(&self, path: &str, content: &str, extra: Value) -> (u16, String) {
        let mut body = json!({"model": GLM_FLASH, "messages": [{"role": "user", "content": content}], "max_tokens": 100});
        for (k, v) in extra.as_object().unwrap() {
            body[k] = v.clone();
        }
        let response = self
            .client
            .post(format!("{}{path}", self.base()))
            .bearer_auth("dummy")
            .header("x-tinyrsi-candidate", "c1")
            .json(&body)
            .send()
            .await
            .unwrap();
        (response.status().as_u16(), response.text().await.unwrap())
    }

    async fn status(&self) -> Value {
        self.client
            .get(format!("{}/status", self.base()))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    fn meter(&self) -> Vec<Value> {
        let text =
            std::fs::read_to_string(self.dir.path().join("run-1/meter.jsonl")).unwrap_or_default();
        text.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    async fn wait_for_meter_rows(&self, rows: usize) {
        for _ in 0..100 {
            if self.meter().len() >= rows {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("meter never reached {rows} rows: {:?}", self.meter());
    }
}

/// Every file under `dir`, recursively.
fn files_under(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn forwards_with_the_real_key_and_charges_reported_cost() {
    let h = Harness::start(5.0, 50.0);
    let tagged = h.gateway.tagged_url("score", "judge");
    let path = tagged.strip_prefix(&h.base()).unwrap();
    let (status, _) = h
        .chat(&format!("{path}/chat/completions"), "hello", json!({}))
        .await;
    assert_eq!(status, 200);
    let seen = h.upstream.seen.lock().unwrap().clone();
    assert_eq!(seen[0].0, format!("Bearer {KEY}"));
    assert_eq!(seen[0].1["provider"]["only"], json!(["z-ai"]));
    assert_eq!(seen[0].1["reasoning"], json!({"effort": "low"}));
    let ledgers = h.status().await;
    assert!((ledgers["run"]["settled_usd"].as_f64().unwrap() - 0.002).abs() < 1e-12);
    assert!((ledgers["total"]["settled_usd"].as_f64().unwrap() - 0.002).abs() < 1e-12);
    let spend = h.gateway.spend();
    assert!((spend.run_usd - 0.002).abs() < 1e-12);
    assert!((spend.total_usd - 0.002).abs() < 1e-12);
    assert!((spend.remaining_usd() - 4.998).abs() < 1e-12);
    let rows = h.meter();
    assert_eq!(rows[0]["phase"], "score");
    assert_eq!(rows[0]["seat"], "judge");
    assert_eq!(rows[0]["candidate"], "c1");
    let captured = std::fs::read_dir(h.dir.path().join("run-1/captures"))
        .unwrap()
        .count();
    assert_eq!(captured, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn streams_through_and_settles_from_the_usage_event() {
    let h = Harness::start(5.0, 50.0);
    let (status, text) = h
        .chat("/v1/chat/completions", "hello", json!({"stream": true}))
        .await;
    assert_eq!(status, 200);
    assert!(text.contains("[DONE]"));
    // Settlement happens after the last byte.
    h.wait_for_meter_rows(1).await;
    let ledgers = h.status().await;
    assert!((ledgers["run"]["settled_usd"].as_f64().unwrap() - 0.001).abs() < 1e-12);
    assert_eq!(h.meter()[0]["stream"], true);
}

#[tokio::test(flavor = "multi_thread")]
async fn refuses_calls_beyond_the_run_cap_with_402() {
    let h = Harness::start(0.01, 50.0);
    let (status, text) = h
        .chat(
            "/v1/chat/completions",
            "hello",
            json!({"max_tokens": 16000}),
        )
        .await;
    assert_eq!(status, 402, "{text}");
    assert!(text.contains("run budget exhausted"));
    assert!(
        h.upstream.seen.lock().unwrap().is_empty(),
        "nothing reached the upstream"
    );
    assert_eq!(h.status().await["run"]["denied"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn refuses_calls_beyond_the_total_cap_with_402() {
    let h = Harness::start(5.0, 0.01);
    let (status, text) = h
        .chat(
            "/v1/chat/completions",
            "hello",
            json!({"max_tokens": 16000}),
        )
        .await;
    assert_eq!(status, 402);
    assert!(text.contains("total budget exhausted"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_breach_halts_later_calls() {
    let h = Harness::start(5.0, 50.0);
    let (status, _) = h.chat("/v1/chat/completions", "overcost", json!({})).await;
    assert_eq!(status, 200);
    let (status, _) = h.chat("/v1/chat/completions", "hello", json!({})).await;
    assert_eq!(status, 402);
}

#[tokio::test(flavor = "multi_thread")]
async fn upstream_rejections_cost_nothing_and_crashes_keep_the_reservation() {
    let h = Harness::start(5.0, 50.0);
    assert_eq!(
        h.chat("/v1/chat/completions", "reject", json!({})).await.0,
        400
    );
    let ledgers = h.status().await;
    assert!(ledgers["run"]["uncertain_usd"].as_f64().unwrap().abs() < 1e-12);
    assert!(ledgers["run"]["settled_usd"].as_f64().unwrap().abs() < 1e-12);
    assert_eq!(
        h.chat("/v1/chat/completions", "crash", json!({})).await.0,
        500
    );
    assert!(h.status().await["run"]["uncertain_usd"].as_f64().unwrap() > 0.0);
    let rows = h.meter();
    assert!(rows[1]["error"].as_str().unwrap().contains("oops"));
}

#[tokio::test(flavor = "multi_thread")]
async fn counts_upstream_refusals_with_their_status_and_message() {
    let h = Harness::start(5.0, 50.0);
    assert_eq!(h.gateway.upstream_calls(), UpstreamCalls::default());
    assert_eq!(
        h.chat("/v1/chat/completions", "hello", json!({})).await.0,
        200
    );
    assert_eq!(
        h.chat("/v1/chat/completions", "crash", json!({})).await.0,
        500
    );
    let before = h.gateway.upstream_calls();
    assert_eq!((before.sent, before.refused), (2, 1));
    assert_eq!(
        before.last_refusal,
        Some(Refusal {
            status: 500,
            message: "oops".into()
        })
    );
    assert_eq!(
        h.chat("/v1/chat/completions", "limit", json!({})).await.0,
        403
    );
    assert_eq!(
        h.chat("/v1/chat/completions", "echo", json!({})).await.0,
        401
    );
    let after = h.gateway.upstream_calls();
    assert_eq!((after.sent, after.refused), (4, 3));
    let echoed = after.last_refusal.clone().unwrap();
    assert_eq!(echoed.status, 401);
    assert!(!echoed.message.contains(KEY), "{}", echoed.message);
    let delta = after.since(&before);
    assert_eq!((delta.sent, delta.refused), (2, 2));
    assert_eq!(
        h.chat("/v1/chat/completions", "limit", json!({})).await.0,
        403
    );
    assert_eq!(
        h.gateway.upstream_calls().last_refusal,
        Some(Refusal {
            status: 403,
            message: "Key limit exceeded (daily limit).".into()
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn calls_the_gateway_refuses_itself_are_not_upstream_calls() {
    let h = Harness::start(0.01, 50.0);
    let (status, _) = h
        .chat(
            "/v1/chat/completions",
            "hello",
            json!({"max_tokens": 16000}),
        )
        .await;
    assert_eq!(status, 402);
    assert_eq!(h.gateway.upstream_calls(), UpstreamCalls::default());
}

#[tokio::test(flavor = "multi_thread")]
async fn prices_token_counts_when_cost_is_missing() {
    let h = Harness::start(5.0, 50.0);
    assert_eq!(
        h.chat("/v1/chat/completions", "tokens", json!({})).await.0,
        200
    );
    let settled = h.status().await["run"]["settled_usd"].as_f64().unwrap();
    assert!((settled - (1000.0 * 0.30e-6 + 1000.0 * 1.0e-6)).abs() < 1e-12);
}

#[tokio::test(flavor = "multi_thread")]
async fn refuses_unknown_models_and_bad_json() {
    let h = Harness::start(5.0, 50.0);
    let (status, _) = h
        .chat(
            "/v1/chat/completions",
            "hello",
            json!({"model": "openai/gpt-5.2"}),
        )
        .await;
    assert_eq!(status, 403);
    let response = h
        .client
        .post(format!("{}/v1/chat/completions", h.base()))
        .body("{")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 400);
    assert_eq!(h.meter().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn serves_embeddings_and_lists_models() {
    let h = Harness::start(5.0, 50.0);
    let tagged = h.gateway.tagged_url("ingest", "memory");
    let response = h
        .client
        .post(format!("{tagged}/embeddings"))
        .json(&json!({"model": "openai/text-embedding-3-small", "input": "x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let response = h
        .client
        .post(format!("{}/v1/embeddings", h.base()))
        .json(&json!({"model": "openai/text-embedding-3-small", "input": "x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    for models_url in [
        format!("{}/v1/models", h.base()),
        format!("{tagged}/models"),
    ] {
        let models: Value = h
            .client
            .get(models_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(models["data"].as_array().unwrap().len(), 2);
    }
    assert_eq!(h.meter()[0]["phase"], "ingest");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_gateway_cannot_share_the_total_ledger() {
    let h = Harness::start(5.0, 50.0);
    let config = GatewayConfig::new(
        "k",
        h.dir.path().join("run-2"),
        h.dir.path().join("ledger-total.json"),
        5.0,
        50.0,
    );
    let error = Gateway::start(config).unwrap_err();
    assert!(matches!(error, Error::Locked(_)), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn unreachable_upstreams_return_502_and_keep_the_reservation() {
    let dir = tempfile::tempdir().unwrap();
    // Bind and release a port so nothing listens on it.
    let dead = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let mut config = GatewayConfig::new(
        KEY,
        dir.path().join("run"),
        dir.path().join("total.json"),
        5.0,
        50.0,
    );
    config.upstream = format!("http://{dead}/v1");
    let gateway = Gateway::start(config).unwrap();
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let response = client
        .post(format!("{}/v1/chat/completions", gateway.base_url()))
        .json(&json!({"model": GLM_FLASH, "messages": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 502);
    assert!(!response.text().await.unwrap().contains(KEY));
    let ledgers: Value = client
        .get(format!("{}/status", gateway.base_url()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(ledgers["run"]["uncertain_usd"].as_f64().unwrap() > 0.0);
    assert!(gateway.spend().run_usd > 0.0);
    let calls = gateway.upstream_calls();
    assert_eq!((calls.sent, calls.refused), (1, 1));
    let refusal = calls.last_refusal.unwrap();
    assert_eq!(refusal.status, 502);
    assert!(!refusal.message.contains(KEY), "{}", refusal.message);
    gateway.shutdown().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_client_that_disconnects_mid_call_still_settles() {
    let h = Harness::start(5.0, 50.0);
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(50))
        .build()
        .unwrap();
    let body = json!({"model": GLM_FLASH, "messages": [{"role": "user", "content": "slow"}]});
    let result = client
        .post(format!("{}/v1/chat/completions", h.base()))
        .json(&body)
        .send()
        .await;
    assert!(
        result.is_err(),
        "the client gives up before the upstream answers"
    );
    for _ in 0..100 {
        let run = h.status().await["run"].clone();
        if run["in_flight_usd"].as_f64().unwrap() < 1e-12 {
            assert!(
                (run["settled_usd"].as_f64().unwrap() - 0.003).abs() < 1e-12,
                "{run}"
            );
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("reservation never settled: {}", h.status().await);
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_waits_for_a_call_still_settling() {
    let h = Harness::start(5.0, 50.0);
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(50))
        .build()
        .unwrap();
    let body = json!({"model": GLM_FLASH, "messages": [{"role": "user", "content": "slow"}]});
    let _ = client
        .post(format!("{}/v1/chat/completions", h.base()))
        .json(&body)
        .send()
        .await;
    let Harness {
        gateway,
        upstream,
        dir,
        ..
    } = h;
    tokio::task::spawn_blocking(move || gateway.shutdown())
        .await
        .unwrap()
        .unwrap();
    drop(upstream);
    let run: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("run-1/budget.json")).unwrap())
            .unwrap();
    assert!(
        run["in_flight_usd"].as_f64().unwrap().abs() < 1e-12,
        "{run}"
    );
    assert!((run["settled_usd"].as_f64().unwrap() - 0.003).abs() < 1e-12);
    assert!(
        !dir.path().join("ledger-total.lock").exists(),
        "shutdown releases the ledger lock"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_key_never_reaches_any_file_or_client() {
    let h = Harness::start(5.0, 50.0);
    for content in ["hello", "reject", "crash", "echo"] {
        let (_, text) = h.chat("/v1/chat/completions", content, json!({})).await;
        assert!(!text.contains(KEY), "{content}: {text}");
    }
    let (_, text) = h
        .chat("/v1/chat/completions", "hello", json!({"stream": true}))
        .await;
    assert!(!text.contains(KEY));
    h.wait_for_meter_rows(5).await;
    assert!(
        h.upstream
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|(auth, _)| auth == &format!("Bearer {KEY}")),
        "the upstream does receive the key"
    );
    let echo_row = &h.meter()[3];
    assert!(
        echo_row["error"].as_str().unwrap().contains("[redacted]"),
        "{echo_row}"
    );
    let files = files_under(h.dir.path());
    assert!(files.iter().any(|f| f.ends_with("budget.json")));
    assert!(files.iter().any(|f| f.ends_with("meter.jsonl")));
    assert!(files.iter().any(|f| f.ends_with("ledger-total.json")));
    assert!(
        files
            .iter()
            .any(|f| f.to_string_lossy().contains("captures"))
    );
    for file in files {
        let bytes = std::fs::read(&file).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(KEY),
            "{} contains the key",
            file.display()
        );
    }
    assert!(!format!("{:?}", h.gateway).contains(KEY));
}

/// A synchronous caller with no async runtime can start, query and stop a
/// gateway.
#[test]
fn works_from_a_caller_without_a_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = GatewayConfig::new(
        KEY,
        dir.path().join("run"),
        dir.path().join("total.json"),
        2.0,
        3.0,
    );
    config.upstream = "http://127.0.0.1:9/v1".into();
    let gateway = Gateway::start(config).unwrap();
    assert_eq!(gateway.base_url(), format!("http://{}", gateway.addr()));
    assert_eq!(
        gateway.tagged_url("a b", "seat/1"),
        format!("{}/__tag/a%20b/seat%2F1/v1", gateway.base_url())
    );
    let mut stream = std::net::TcpStream::connect(gateway.addr()).unwrap();
    stream
        .write_all(b"GET /status HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains("\"limit_usd\":2.0"), "{response}");
    let spend = gateway.spend();
    assert!((spend.run_limit_usd - 2.0).abs() < 1e-12);
    assert!((spend.total_limit_usd - 3.0).abs() < 1e-12);
    assert!((spend.remaining_usd() - 2.0).abs() < 1e-12);
    gateway.shutdown().unwrap();
    assert!(!dir.path().join("total.lock").exists());
}

/// Dropping the handle stops the gateway and frees the ledger for the next
/// run, which carries the spend forward.
#[test]
fn drop_stops_the_gateway_and_releases_the_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let config = || {
        let mut config = GatewayConfig::new(
            KEY,
            dir.path().join("run"),
            dir.path().join("total.json"),
            2.0,
            3.0,
        );
        config.upstream = "http://127.0.0.1:9/v1".into();
        config
    };
    let addr = {
        let gateway = Gateway::start(config()).unwrap();
        gateway.addr()
    };
    assert!(std::net::TcpStream::connect(addr).is_err());
    let again = Gateway::start(config()).unwrap();
    assert!((again.spend().total_limit_usd - 3.0).abs() < 1e-12);
}
