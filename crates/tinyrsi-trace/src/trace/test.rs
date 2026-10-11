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
fn ideated_carries_an_optional_approach_label() {
    let labelled = LoopEvent::Ideated {
        epoch: 3,
        proposal_id: "p3-1".into(),
        parent: "a".into(),
        candidate: Some("b".into()),
        rationale: "r".into(),
        hypothesis: "h".into(),
        approach: Some("tabu search".into()),
    };
    let text = r#"{"kind":"ideated","epoch":3,"proposal_id":"p3-1","parent":"a","candidate":"b","rationale":"r","hypothesis":"h","approach":"tabu search"}"#;
    assert_eq!(serde_json::to_string(&labelled).unwrap(), text);
    assert_eq!(serde_json::from_str::<LoopEvent>(text).unwrap(), labelled);
    // A missing field and an explicit null both read as no label.
    for absent in [
        r#"{"kind":"ideated","epoch":3,"proposal_id":"p3-1","parent":"a","candidate":"b","rationale":"r","hypothesis":"h"}"#,
        r#"{"kind":"ideated","epoch":3,"proposal_id":"p3-1","parent":"a","candidate":"b","rationale":"r","hypothesis":"h","approach":null}"#,
    ] {
        assert!(
            matches!(
                serde_json::from_str(absent).unwrap(),
                LoopEvent::Ideated { approach: None, .. }
            ),
            "{absent}"
        );
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
        approach: None,
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

#[test]
fn scored_names_quality_units_and_omits_metric_units() {
    let scored = |units| LoopEvent::Scored {
        epoch: 1,
        candidate: "c".into(),
        decision: ScoreDecision::Accept,
        estimate: 0.5,
        lower: 0.25,
        upper: 0.75,
        units,
    };
    let metric = serde_json::to_string(&scored(IntervalUnits::Metric)).unwrap();
    assert_eq!(
        metric,
        r#"{"kind":"scored","epoch":1,"candidate":"c","decision":"accept","estimate":0.5,"lower":0.25,"upper":0.75}"#
    );
    let quality = serde_json::to_string(&scored(IntervalUnits::Quality)).unwrap();
    assert_eq!(
        quality,
        r#"{"kind":"scored","epoch":1,"candidate":"c","decision":"accept","estimate":0.5,"lower":0.25,"upper":0.75,"units":"quality"}"#
    );
    // Traces written before the field existed read as metric units.
    assert_eq!(
        serde_json::from_str::<LoopEvent>(&metric).unwrap(),
        scored(IntervalUnits::Metric)
    );
    assert_eq!(
        serde_json::from_str::<LoopEvent>(&quality).unwrap(),
        scored(IntervalUnits::Quality)
    );
}
