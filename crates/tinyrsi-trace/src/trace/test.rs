//! Wire-shape pins and sink behavior for the loop trace.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::*;

/// The example run from `docs/specs/loop-trace.md`, byte for byte.
const EXAMPLE: &str = include_str!("example.jsonl");

#[test]
fn every_specified_example_line_round_trips_byte_for_byte() {
    let lines: Vec<&str> = EXAMPLE.lines().collect();
    assert_eq!(lines.len(), 12);
    let spec = include_str!("../../../../docs/specs/loop-trace.md");
    for (seq, line) in lines.iter().enumerate() {
        assert!(spec.contains(line), "spec no longer shows {line}");
        let record: TraceRecord = serde_json::from_str(line).unwrap();
        assert_eq!(record.seq, u64::try_from(seq).unwrap());
        assert_eq!(serde_json::to_string(&record).unwrap(), *line);
    }
}

#[test]
fn covers_the_observed_and_failed_execution_shapes() {
    let observed = LoopEvent::Observed {
        epoch: 1,
        incumbent: "a".into(),
        incumbent_metric: None,
        history: 2,
        rejected: 3,
    };
    assert_eq!(
        serde_json::to_string(&observed).unwrap(),
        r#"{"kind":"observed","epoch":1,"incumbent":"a","incumbent_metric":null,"history":2,"rejected":3}"#
    );
    let failed = LoopEvent::Executed {
        epoch: 0,
        candidate: "a".into(),
        seed: 7,
        metric: None,
        ok: false,
        duration_ms: 4,
    };
    assert_eq!(
        serde_json::to_string(&failed).unwrap(),
        r#"{"kind":"executed","epoch":0,"candidate":"a","seed":7,"metric":null,"ok":false,"duration_ms":4}"#
    );
    let refused = LoopEvent::Ideated {
        epoch: 2,
        proposal_id: "p2-0".into(),
        parent: "a".into(),
        candidate: None,
        rationale: "r".into(),
        hypothesis: String::new(),
    };
    assert_eq!(
        serde_json::to_string(&refused).unwrap(),
        r#"{"kind":"ideated","epoch":2,"proposal_id":"p2-0","parent":"a","candidate":null,"rationale":"r","hypothesis":""}"#
    );
    for (reason, text) in [
        (StopReason::GoalReached, "goal_reached"),
        (StopReason::Stalled, "stalled"),
        (StopReason::Budget, "budget"),
        (StopReason::Error, "error"),
    ] {
        assert_eq!(
            serde_json::to_string(&reason).unwrap(),
            format!("\"{text}\"")
        );
    }
    for (decision, text) in [
        (ScoreDecision::Reject, "reject"),
        (ScoreDecision::Inconclusive, "inconclusive"),
    ] {
        assert_eq!(
            serde_json::to_string(&decision).unwrap(),
            format!("\"{text}\"")
        );
    }
    assert_eq!(
        serde_json::to_string(&MetricDirection::Maximize).unwrap(),
        "\"maximize\""
    );
}

fn started() -> LoopEvent {
    LoopEvent::EpochStarted {
        epoch: 1,
        incumbent: "a".into(),
    }
}

#[test]
fn manual_clock_clones_share_time() {
    let clock = ManualClock::new();
    let shared = clock.clone();
    clock.advance(5);
    shared.advance(2);
    assert_eq!(clock.elapsed_ms(), 7);
    assert!(MonotonicClock::default().elapsed_ms() < 60_000);
}

#[test]
fn jsonl_trace_numbers_flushes_and_terminates_every_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/run/events.jsonl");
    let clock = ManualClock::new();
    let mut trace = JsonlTrace::create(&path, clock.clone()).unwrap();
    trace.record(started()).unwrap();
    clock.advance(9);
    trace.record(started()).unwrap();
    // Read while the writer is still open: every line is already visible.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.ends_with('\n'));
    let records: Vec<TraceRecord> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(records.len(), 2);
    assert_eq!((records[0].seq, records[0].elapsed_ms), (0, 0));
    assert_eq!((records[1].seq, records[1].elapsed_ms), (1, 9));
    assert!(format!("{trace:?}").contains("seq: 2"));
}

#[test]
fn jsonl_trace_refuses_to_reuse_an_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    std::fs::write(&path, "").unwrap();
    assert_eq!(
        JsonlTrace::create(&path, ManualClock::new()).unwrap_err(),
        Error::Io
    );
}

#[test]
fn vec_and_null_traces_accept_events() {
    let mut trace = VecTrace::new(ManualClock::new());
    trace.record(started()).unwrap();
    trace.record(started()).unwrap();
    assert_eq!(trace.records()[1].seq, 1);
    assert!(format!("{trace:?}").contains("records: 2"));
    assert!(NullTrace.record(started()).is_ok());
}
