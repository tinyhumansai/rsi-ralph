//! Budget and secret projection regressions.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::*;
fn request() -> ModelRequest {
    ModelRequest::new(
        "private-system".into(),
        "private-task".into(),
        42,
        2048,
        100,
        1000,
    )
    .unwrap()
}
#[test]
fn rejects_insufficient_reservations_before_provider_execution() {
    assert!(ModelRequest::new("s".into(), "p".into(), 0, 10, 1, 1).is_err());
    assert!(ModelRequest::new(String::new(), "p".into(), 0, 2048, 1, 1).is_err());
    assert!(
        request()
            .with_tool("bad-name".into(), serde_json::json!({}))
            .is_err()
    );
    assert!(!format!("{:?}", request()).contains("private"));
    assert_eq!(request().seed(), 42);
    assert_eq!(request().system(), "private-system");
    assert_eq!(request().prompt(), "private-task");
    assert_eq!(request().max_tokens(), 2048);
    assert_eq!(request().max_output_tokens(), 100);
}
#[test]
fn exposes_the_structured_tool_to_external_ports() {
    assert_eq!(request().tool(), None);
    let schema = serde_json::json!({"type":"object","required":["score"]});
    let req = request().with_tool("grade".into(), schema.clone()).unwrap();
    assert_eq!(req.tool(), Some(("grade", &schema)));
}
#[test]
fn verifies_authoritative_usage_and_native_tool_identity() {
    let req = request();
    assert!(
        req.check(&ModelReply {
            content: vec![1],
            input_tokens: 10,
            output_tokens: 101
        })
        .is_err()
    );
    let value = serde_json::json!({"choices":[{"message":{"content":"ok"}}],"usage":{"prompt_tokens":10,"completion_tokens":1}});
    assert_eq!(
        OpenRouterModel::decode(&req, &value).unwrap().content,
        b"ok"
    );
    let req = req
        .with_tool("answer".into(), serde_json::json!({"type":"object"}))
        .unwrap();
    assert!(OpenRouterModel::decode(&req, &value).is_err());
    let model = OpenRouterModel::new("model".into()).unwrap();
    assert_eq!(
        model.body(&req)["tool_choice"]["function"]["name"],
        "answer"
    );
    let value = serde_json::json!({"choices":[{"message":{"tool_calls":[{"function":{"name":"answer","arguments":"{}"}}]}}],"usage":{"prompt_tokens":10,"completion_tokens":1}});
    assert_eq!(
        OpenRouterModel::decode(&req, &value).unwrap().content,
        b"{}"
    );
    assert!(OpenRouterModel::new("\n".into()).is_err());
}
struct Script;
impl ModelPort for Script {
    fn generate(&self, _: ModelRequest) -> ModelFuture {
        Box::pin(async {
            Ok(ModelReply {
                content: b"{}".to_vec(),
                input_tokens: 10,
                output_tokens: 1,
            })
        })
    }
}
#[tokio::test]
async fn charges_every_call_before_execution_and_never_exceeds_total() {
    use tinybackrooms::Conversant;
    let backend = ModelConversant::new(Arc::new(Script), 0, 1024, 1024, 100, 1000).unwrap();
    let r = tinybackrooms::TurnRequest {
        seat: tinybackrooms::Seat {
            run_id: "v0".into(),
            role: "a".into(),
        },
        model: None,
        temperature: 0.0,
        system: "s".into(),
        messages: vec![tinybackrooms::Message::user("p")],
    };
    assert!(backend.turn(r.clone()).await.is_ok());
    assert!(backend.turn(r).await.is_err());
    let m = backend.meter().unwrap();
    assert_eq!(m.attempted_calls, 1);
    assert_eq!(m.denied_calls, 1);
    assert_eq!(m.input_tokens, 10);
}
#[test]
fn https_adapter_decodes_usage_without_exposing_prompts_or_credentials_in_argv() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn response(status: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let reply = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut data = Vec::new();
            let mut buf = [0; 8192];
            loop {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
                if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&data[..end]);
                    let length = head
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|n| n.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if data.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            stream.write_all(reply.as_bytes()).unwrap();
        });
        format!("http://{addr}")
    }
    let model = OpenRouterModel::new("test".into()).unwrap();
    let body = r#"{"choices":[{"message":{"content":"ok"}}],"usage":{"prompt_tokens":10,"completion_tokens":1}}"#;
    let url = response("200 OK", body);
    assert_eq!(
        model
            .call_with_key(&request(), "dummy-key", &url)
            .unwrap()
            .content,
        b"ok"
    );
    assert!(
        model
            .call_with_key(&request(), "dummy-key", &response("500 Failed", "withheld"))
            .is_err()
    );
    assert!(
        model
            .call_with_key(&request(), "dummy-key", &response("200 OK", "not-json"))
            .is_err()
    );
    assert!(
        model
            .call_with_key(&request(), "bad\nkey", "http://127.0.0.1")
            .is_err()
    );
}
#[test]
fn a_gateway_adapter_posts_to_the_base_url_with_a_placeholder_token() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (seen, heard) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(300)))
            .unwrap();
        let mut data = Vec::new();
        let mut buf = [0; 8192];
        while let Ok(n) = stream.read(&mut buf) {
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
            if data.windows(4).any(|w| w == b"\r\n\r\n") && data.ends_with(b"}") {
                break;
            }
        }
        seen.send(String::from_utf8_lossy(&data).into_owned())
            .unwrap();
        let body = r#"{"choices":[{"message":{"content":"ok"}}],"usage":{"prompt_tokens":3,"completion_tokens":1}}"#;
        let reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(reply.as_bytes()).unwrap();
    });
    let model =
        OpenRouterModel::via("m".into(), &format!("http://{addr}/__tag/judge/q/v1/")).unwrap();
    assert_eq!(model.call(&request()).unwrap().content, b"ok");
    let head = heard.recv().unwrap();
    assert!(head.starts_with("POST /__tag/judge/q/v1/chat/completions "));
    assert!(head.contains("Authorization: Bearer gateway"));
    for bad in ["ftp://x", "http://a\"b", "http://a\nb"] {
        assert!(OpenRouterModel::via("m".into(), bad).is_err());
    }
}
struct Fails;
impl ModelPort for Fails {
    fn generate(&self, _: ModelRequest) -> ModelFuture {
        Box::pin(async { Err(Error::CallFailed) })
    }
}
#[tokio::test]
async fn failed_calls_keep_their_reservation_and_structured_calls_use_native_tools() {
    use tinybackrooms::Conversant;
    let r = tinybackrooms::StructuredRequest {
        seat: tinybackrooms::Seat {
            run_id: "v0".into(),
            role: "formatter".into(),
        },
        model: None,
        system: "s".into(),
        messages: vec![tinybackrooms::Message::user("p")],
        tool: "answer".into(),
        schema: serde_json::json!({"type":"object"}),
    };
    let fail = ModelConversant::new(Arc::new(Fails), 0, 4096, 2048, 100, 1000).unwrap();
    assert!(fail.structured(r.clone()).await.is_err());
    let m = fail.meter().unwrap();
    assert_eq!(m.failed_calls, 1);
    assert!(m.reserved_tokens > 0);
    let pass = ModelConversant::new(Arc::new(Script), 0, 4096, 2048, 100, 1000).unwrap();
    assert_eq!(
        pass.structured(r.clone()).await.unwrap().value,
        serde_json::json!({})
    );
    let mut override_model = r;
    override_model.model = Some("unbound-model".into());
    assert!(pass.structured(override_model).await.is_err());
    assert!(!format!("{pass:?}").contains("private"));
    assert!(
        !format!(
            "{:?}",
            ModelReply {
                content: b"private".to_vec(),
                input_tokens: 1,
                output_tokens: 1
            }
        )
        .contains("private")
    );
    assert!(ModelConversant::new(Arc::new(Script), 0, 0, 1, 1, 1).is_err());
}
#[tokio::test]
async fn provider_budget_covers_separate_stages_and_charges_failed_requests() {
    let budget = BudgetedModel::new(Arc::new(Script), request().reservation()).unwrap();
    assert!(budget.generate(request()).await.is_ok());
    assert!(budget.generate(request()).await.is_err());
    assert_eq!(budget.meter().unwrap().denied_calls, 1);
    let budget = BudgetedModel::new(Arc::new(Fails), request().reservation()).unwrap();
    assert!(budget.generate(request()).await.is_err());
    assert!(budget.generate(request()).await.is_err());
    assert_eq!(budget.meter().unwrap().attempted_calls, 1);
    assert_eq!(budget.meter().unwrap().failed_calls, 1);
    assert!(!format!("{budget:?}").contains("private"));
    assert_eq!(request().timeout_ms(), 1000);
}
#[tokio::test]
async fn dedicated_worker_wakes_an_executor_and_propagates_provider_errors() {
    let reply = ModelWorker::start(|| {
        std::thread::sleep(std::time::Duration::from_millis(1));
        Ok(ModelReply {
            content: b"ok".to_vec(),
            input_tokens: 1,
            output_tokens: 1,
        })
    })
    .await
    .unwrap();
    assert_eq!(reply.content, b"ok");
    assert!(ModelWorker::start(|| Err(Error::CallFailed)).await.is_err());
}

#[test]
fn public_provider_identity_survives_budget_wrapping_without_provider_calls() {
    let provider = OpenRouterModel::new("openai/frozen-test-model".into()).unwrap();
    let expected = Some(ModelIdentity {
        provider: "openrouter".into(),
        model: "openai/frozen-test-model".into(),
    });
    assert_eq!(provider.identity(), expected);
    let wrapped = BudgetedModel::new(Arc::new(provider), 1000).unwrap();
    assert_eq!(wrapped.identity(), expected);
    assert_eq!(wrapped.meter().unwrap().attempted_calls, 0);
}
#[test]
fn a_request_temperature_reaches_the_provider_body() {
    let model = OpenRouterModel::new("m".into()).unwrap();
    assert_eq!(
        model.body(&request())["temperature"],
        serde_json::json!(0.0)
    );
    let warm = request().with_temperature(1.2).unwrap();
    assert_eq!(warm.temperature(), Some(1.2));
    assert_eq!(model.body(&warm)["temperature"], serde_json::json!(1.2));
    assert!(request().with_temperature(2.5).is_err());
    assert!(request().with_temperature(-0.1).is_err());
}
