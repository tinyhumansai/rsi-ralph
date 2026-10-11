//! Mapping, batching and delivery of the Langfuse sink, against a loopback
//! HTTP listener so no test touches the network.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use tinyrsi_trace::{MetricDirection, RunGoal, ScoreDecision, StopReason};

fn run() -> Vec<LoopEvent> {
    vec![
        LoopEvent::RunStarted {
            run_id: "r1".into(),
            subject: "solver".into(),
            goal: RunGoal {
                metric: "error".into(),
                direction: MetricDirection::Minimize,
                target: Some(0.0),
            },
        },
        LoopEvent::EpochStarted {
            epoch: 1,
            incumbent: "a".into(),
        },
        LoopEvent::Scored {
            epoch: 1,
            candidate: "b".into(),
            decision: ScoreDecision::Accept,
            estimate: 0.5,
            lower: 0.1,
            upper: 0.9,
        },
        LoopEvent::EpochFinished {
            epoch: 1,
            incumbent: "b".into(),
            incumbent_metric: Some(0.25),
        },
        LoopEvent::RunFinished {
            reason: StopReason::MaxEpochs,
            incumbent: "b".into(),
            incumbent_metric: Some(0.25),
        },
    ]
}

/// One received request: request line, headers (lowercased names) and body.
struct Request {
    line: String,
    headers: Vec<(String, String)>,
    body: Value,
}

/// Serves `responses.len()` requests on a loopback port, answering each with
/// the given status, and forwards what it received.
fn listener(responses: Vec<u16>) -> (String, mpsc::Receiver<Request>) {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", socket.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for status in responses {
            let (stream, _) = socket.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut headers = vec![];
            loop {
                let mut h = String::new();
                reader.read_line(&mut h).unwrap();
                if h.trim().is_empty() {
                    break;
                }
                let (k, v) = h.split_once(':').unwrap();
                headers.push((k.trim().to_ascii_lowercase(), v.trim().to_owned()));
            }
            let len: usize = headers
                .iter()
                .find(|(k, _)| k == "content-length")
                .map_or(0, |(_, v)| v.parse().unwrap());
            let mut body = vec![0; len];
            reader.read_exact(&mut body).unwrap();
            let reply = "{}";
            let mut stream = reader.into_inner();
            write!(
                stream,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            )
            .unwrap();
            tx.send(Request {
                line: line.trim().to_owned(),
                headers,
                body: serde_json::from_slice(&body).unwrap(),
            })
            .unwrap();
        }
    });
    (base, rx)
}

fn sink(base: &str) -> LangfuseTrace {
    LangfuseTrace::new(LangfuseClient::direct(base, "pk-test", "sk-test").unwrap()).unwrap()
}

#[test]
fn formats_unix_milliseconds_as_utc_rfc3339() {
    assert_eq!(time::iso8601(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(time::iso8601(951_782_400_000), "2000-02-29T00:00:00.000Z");
    assert_eq!(time::iso8601(1_791_703_984_123), "2026-10-11T07:33:04.123Z");
    assert_eq!(time::iso8601(4_102_444_799_999), "2099-12-31T23:59:59.999Z");
}

#[test]
fn maps_a_run_to_one_trace_with_an_epoch_span_and_nested_events() {
    let mut builder = BatchBuilder::default();
    let events: Vec<Value> = run().iter().flat_map(|e| builder.push(e, 0)).collect();
    let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(
        types,
        [
            "trace-create",
            "span-create",
            "event-create",
            "event-create",
            "span-update",
            "event-create",
            "trace-create",
            "score-create",
        ]
    );
    let ids: std::collections::BTreeSet<&str> =
        events.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), events.len(), "envelope ids are unique");
    assert_eq!(events[0]["body"]["id"], "r1");
    assert_eq!(events[0]["body"]["input"]["goal"]["metric"], "error");
    assert_eq!(events[1]["body"]["id"], "r1:epoch:1");
    assert_eq!(events[2]["body"]["name"], "scored");
    assert_eq!(events[2]["body"]["parentObservationId"], "r1:epoch:1");
    assert_eq!(events[2]["body"]["input"]["decision"], "accept");
    assert_eq!(events[4]["body"]["endTime"], "1970-01-01T00:00:00.000Z");
    assert!(events[5]["body"].get("parentObservationId").is_none());
    assert_eq!(events[6]["body"]["output"]["reason"], "max_epochs");
    assert_eq!(events[7]["body"]["value"], 0.25);
    assert!(
        events
            .iter()
            .all(|e| e["body"]["traceId"] == "r1" || e["type"] == "trace-create")
    );
}

#[test]
fn ignores_events_outside_a_run_and_skips_a_missing_final_metric() {
    let mut builder = BatchBuilder::default();
    assert_eq!(builder.push(&run()[2], 0), Vec::<Value>::new());
    builder.push(&run()[0], 0);
    let finished = builder.push(
        &LoopEvent::RunFinished {
            reason: StopReason::Error,
            incumbent: "a".into(),
            incumbent_metric: None,
        },
        0,
    );
    assert_eq!(finished.len(), 2);
    assert_eq!(
        builder.push(&run()[1], 0),
        Vec::<Value>::new(),
        "the run is closed"
    );
}

#[test]
fn delivers_the_run_with_basic_auth_when_it_finishes() {
    let (base, rx) = listener(vec![200]);
    let mut trace = sink(&base);
    for event in run() {
        trace.record(event).unwrap();
    }
    let request = rx.recv().unwrap();
    assert_eq!(request.line, "POST /api/public/ingestion HTTP/1.1");
    let auth = request
        .headers
        .iter()
        .find(|(k, _)| k == "authorization")
        .map(|(_, v)| v.as_str());
    // base64("pk-test:sk-test")
    assert_eq!(auth, Some("Basic cGstdGVzdDpzay10ZXN0"));
    assert_eq!(request.body["batch"].as_array().unwrap().len(), 8);
    assert_eq!(trace.pending(), [] as [Value; 0]);
    assert_eq!(trace.failures(), 0);
    assert!(format!("{trace:?}").contains("pending: 0"));
}

#[test]
fn a_failed_export_is_counted_kept_and_retried() {
    let (base, rx) = listener(vec![500, 200]);
    let mut trace = sink(&base);
    for event in run() {
        trace.record(event).unwrap();
    }
    rx.recv().unwrap();
    assert_eq!(trace.failures(), 1);
    assert_eq!(trace.pending().len(), 8);
    trace.flush().unwrap();
    assert_eq!(
        rx.recv().unwrap().body["batch"].as_array().unwrap().len(),
        8
    );
    assert_eq!(trace.pending(), [] as [Value; 0]);
}

#[test]
fn an_explicit_flush_reports_the_failure() {
    let (base, _rx) = listener(vec![500]);
    let mut trace = sink(&base);
    trace.record(run()[0].clone()).unwrap();
    assert_eq!(trace.flush(), Err(Error::ExportFailed));
    assert_eq!(trace.pending().len(), 1);
}

#[test]
fn splits_large_runs_into_batches_of_at_most_the_ingestion_limit() {
    let (base, rx) = listener(vec![200, 200]);
    let mut trace = sink(&base);
    trace.record(run()[0].clone()).unwrap();
    for epoch in 0..MAX_BATCH {
        trace
            .record(LoopEvent::EpochStarted {
                epoch: u64::try_from(epoch).unwrap(),
                incumbent: "a".into(),
            })
            .unwrap();
    }
    trace.flush().unwrap();
    let sizes =
        [rx.recv().unwrap(), rx.recv().unwrap()].map(|r| r.body["batch"].as_array().unwrap().len());
    assert_eq!(sizes, [MAX_BATCH, 1]);
}

#[test]
fn flushing_from_inside_a_tokio_runtime_does_not_nest_runtimes() {
    let (base, rx) = listener(vec![200]);
    let mut trace = sink(&base);
    let outer = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    outer.block_on(async {
        for event in run() {
            trace.record(event).unwrap();
        }
    });
    assert_eq!(
        rx.recv().unwrap().body["batch"].as_array().unwrap().len(),
        8
    );
    assert_eq!(trace.failures(), 0);
}
