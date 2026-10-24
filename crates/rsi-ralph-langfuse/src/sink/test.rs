//! Span mapping and OTLP delivery of the Langfuse sink, against a loopback
//! HTTP listener so no test touches the network.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::spans::SpanBuilder;
use super::*;
use openhuman_embed::observe::langfuse::{SpanKind, SpanStatus};
use rsi_ralph_trace::{IntervalUnits, MetricDirection, RunGoal, ScoreDecision, StopReason};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

fn run(reason: StopReason) -> Vec<LoopEvent> {
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
            units: IntervalUnits::Metric,
        },
        LoopEvent::EpochFinished {
            epoch: 1,
            incumbent: "b".into(),
            incumbent_metric: Some(0.25),
        },
        LoopEvent::RunFinished {
            reason,
            incumbent: "b".into(),
            incumbent_metric: Some(0.25),
        },
    ]
}

/// One received request: request line, authorization header and body.
struct Request {
    line: String,
    auth: String,
    body: Value,
}

/// Serves one request per entry of `statuses` on a loopback port, each
/// answered with an empty JSON object.
fn listener(statuses: Vec<u16>) -> (String, mpsc::Receiver<Request>) {
    responding(statuses.into_iter().map(|status| (status, "{}")).collect())
}

/// Serves one request per `(status, body)` entry on a loopback port.
fn responding(responses: Vec<(u16, &'static str)>) -> (String, mpsc::Receiver<Request>) {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", socket.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for (status, reply) in responses {
            let (stream, _) = socket.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let (mut auth, mut len) = (String::new(), 0_usize);
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header.trim().is_empty() {
                    break;
                }
                let (name, value) = header.split_once(':').unwrap();
                match name.trim().to_ascii_lowercase().as_str() {
                    "authorization" => value.trim().clone_into(&mut auth),
                    "content-length" => len = value.trim().parse().unwrap(),
                    _ => {}
                }
            }
            let mut body = vec![0; len];
            reader.read_exact(&mut body).unwrap();
            let mut stream = reader.into_inner();
            write!(
                stream,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            )
            .unwrap();
            tx.send(Request {
                line: line.trim().to_owned(),
                auth,
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

/// Every span in an OTLP request body.
fn spans_in(body: &Value) -> Vec<Value> {
    body["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn maps_a_run_to_a_root_span_with_an_epoch_and_stage_spans() {
    let mut builder = SpanBuilder::default();
    let mut finished = None;
    for (i, event) in run(StopReason::MaxEpochs).iter().enumerate() {
        finished = builder.push(event, 100 + u64::try_from(i).unwrap());
    }
    let spans = finished.unwrap();
    let names: Vec<&str> = spans.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "tinyrsi.run",
            "epoch 1",
            "scored",
            "epoch_finished",
            "run_finished"
        ]
    );
    let root = &spans[0];
    assert_eq!((root.kind, root.status), (SpanKind::Turn, SpanStatus::Ok));
    assert!(root.parent_span_id.is_none());
    assert_eq!(root.input.as_ref().unwrap()["goal"]["metric"], "error");
    assert_eq!(root.output.as_ref().unwrap()["incumbent_metric"], 0.25);
    assert_eq!(root.end_unix_ms, Some(104));
    let epoch = &spans[1];
    assert_eq!(epoch.parent_span_id.as_deref(), Some("r1"));
    assert_eq!((epoch.start_unix_ms, epoch.end_unix_ms), (101, Some(103)));
    assert_eq!(spans[2].parent_span_id.as_deref(), Some("r1:epoch:1"));
    assert_eq!(spans[2].input.as_ref().unwrap()["decision"], "accept");
    assert_eq!(spans[4].parent_span_id.as_deref(), Some("r1"));
    assert!(spans.iter().all(|s| s.trace_id == "r1"));
}

#[test]
fn a_failed_run_ends_its_root_in_error_and_events_outside_a_run_are_dropped() {
    let mut builder = SpanBuilder::default();
    assert!(builder.push(&run(StopReason::Error)[2], 0).is_none());
    let mut finished = None;
    for event in run(StopReason::Error) {
        finished = builder.push(&event, 0);
    }
    assert_eq!(finished.unwrap()[0].status, SpanStatus::Error);
    assert!(
        builder.push(&run(StopReason::Error)[1], 0).is_none(),
        "run closed"
    );
}

#[test]
fn delivers_the_finished_run_as_otlp_with_basic_auth() {
    let (base, rx) = listener(vec![200]);
    let mut trace = sink(&base).with_environment("production");
    for event in run(StopReason::MaxEpochs) {
        trace.record(event).unwrap();
    }
    let request = rx.recv().unwrap();
    assert_eq!(request.line, "POST /api/public/otel/v1/traces HTTP/1.1");
    // base64("pk-test:sk-test")
    assert_eq!(request.auth, "Basic cGstdGVzdDpzay10ZXN0");
    assert_eq!(spans_in(&request.body).len(), 5);
    assert!(request.body.to_string().contains("production"));
    assert_eq!(trace.pending(), [] as [Value; 0]);
    assert_eq!(trace.failures(), 0);
    assert!(format!("{trace:?}").contains("/api/public/otel/v1/traces"));
}

#[test]
fn a_failed_export_is_counted_kept_and_retried() {
    let (base, rx) = listener(vec![500, 200]);
    let mut trace = sink(&base);
    for event in run(StopReason::MaxEpochs) {
        trace.record(event).unwrap();
    }
    rx.recv().unwrap();
    assert_eq!(trace.failures(), 1);
    assert_eq!(trace.pending().len(), 1);
    trace.flush().unwrap();
    assert_eq!(spans_in(&rx.recv().unwrap().body).len(), 5);
    assert_eq!(trace.pending(), [] as [Value; 0]);
}

#[test]
fn an_explicit_flush_reports_the_failure() {
    let (base, _rx) = listener(vec![500]);
    let mut trace = sink(&base);
    for event in run(StopReason::MaxEpochs) {
        trace.record(event).unwrap();
    }
    assert_eq!(trace.failures(), 1);
    let (base, _rx) = listener(vec![413]);
    let mut other = sink(&base);
    other.pending.clone_from(&trace.pending);
    assert_eq!(other.flush(), Err(Error::ExportFailed));
    assert_eq!(other.pending().len(), 1);
}

#[test]
fn a_partially_rejected_batch_counts_as_a_failure() {
    let (base, rx) = responding(vec![(
        200,
        r#"{"partialSuccess":{"rejectedSpans":"2","errorMessage":"x"}}"#,
    )]);
    let mut trace = sink(&base);
    for event in run(StopReason::MaxEpochs) {
        trace.record(event).unwrap();
    }
    rx.recv().unwrap();
    assert_eq!(trace.failures(), 1);
    // The accepted spans are not sent again.
    assert_eq!(trace.pending(), [] as [Value; 0]);
}

#[test]
fn a_snake_case_partial_rejection_also_counts_as_a_failure() {
    let (base, _rx) = responding(vec![(200, r#"{"partial_success":{"rejected_spans":3}}"#)]);
    let mut trace = sink(&base);
    for event in run(StopReason::MaxEpochs) {
        trace.record(event).unwrap();
    }
    assert_eq!(trace.failures(), 1);
    assert_eq!(trace.pending(), [] as [Value; 0]);
}

#[test]
fn a_partial_success_with_nothing_rejected_counts_as_success() {
    let (base, _rx) = responding(vec![(
        200,
        r#"{"partialSuccess":{"rejectedSpans":"0","errorMessage":""}}"#,
    )]);
    let mut trace = sink(&base);
    for event in run(StopReason::MaxEpochs) {
        trace.record(event).unwrap();
    }
    assert_eq!(trace.failures(), 0);
    assert_eq!(trace.pending(), [] as [Value; 0]);
}

#[test]
fn a_long_run_is_split_into_several_requests() {
    let (base, rx) = listener(vec![200, 200]);
    let mut trace = sink(&base);
    let mut events = run(StopReason::MaxEpochs);
    let finish = events.pop().unwrap();
    for _ in 0..25 {
        events.insert(2, events[2].clone());
    }
    for event in events.into_iter().chain([finish]) {
        trace.record(event).unwrap();
    }
    let sizes = [rx.recv().unwrap(), rx.recv().unwrap()].map(|r| spans_in(&r.body).len());
    assert_eq!(sizes.iter().sum::<usize>(), 30);
    assert!(sizes.iter().all(|n| *n > 0));
}

#[test]
fn the_sink_can_be_used_and_dropped_inside_a_tokio_runtime() {
    let (base, rx) = listener(vec![200]);
    let outer = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    outer.block_on(async {
        let mut trace = sink(&base);
        for event in run(StopReason::MaxEpochs) {
            trace.record(event).unwrap();
        }
        assert_eq!(trace.failures(), 0);
        drop(trace);
    });
    assert_eq!(spans_in(&rx.recv().unwrap().body).len(), 5);
}

#[test]
fn an_epoch_interrupted_by_a_failed_run_ends_in_error() {
    let mut builder = SpanBuilder::default();
    let events = run(StopReason::Error);
    // Fail mid-epoch: started, scored, then the run ends without the epoch.
    let mut finished = None;
    for event in [&events[0], &events[1], &events[2], &events[4]] {
        finished = builder.push(event, 0);
    }
    let spans = finished.unwrap();
    let epoch = spans.iter().find(|s| s.name == "epoch 1").unwrap();
    assert_eq!(epoch.status, SpanStatus::Error);
    assert_eq!(spans[0].status, SpanStatus::Error);
    // Stage spans that completed keep their own status.
    let scored = spans.iter().find(|s| s.name == "scored").unwrap();
    assert_eq!(scored.status, SpanStatus::Ok);
}

#[test]
fn a_run_stopped_by_provider_refusals_ends_in_error() {
    let mut builder = SpanBuilder::default();
    let mut finished = None;
    for event in run(StopReason::ProviderRefused) {
        finished = builder.push(&event, 0);
    }
    let spans = finished.unwrap();
    assert_eq!(spans[0].status, SpanStatus::Error);
}
