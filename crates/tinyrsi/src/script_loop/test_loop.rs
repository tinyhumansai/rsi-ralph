//! End-to-end loop tests with `/bin/sh` fixtures and a scripted supervisor.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "assert deterministic test fixtures"
)]
use super::test::{Scripted, loss, reply, subject, supervisor};
use super::*;
use crate::{JsonlTrace, LoopEvent, ManualClock, TraceRecord, VecTrace};
use std::path::Path;

fn location(dir: &Path) -> RunLocation {
    RunLocation {
        id: "run-1".into(),
        dir: dir.join("run"),
    }
}

fn policy(max_epochs: u64, proposals_per_epoch: u32) -> LoopPolicy {
    LoopPolicy {
        max_epochs,
        proposals_per_epoch,
        ..LoopPolicy::default()
    }
}

fn kinds(trace: &VecTrace) -> Vec<String> {
    trace
        .records()
        .iter()
        .map(|r| {
            serde_json::to_value(&r.event).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

fn ledger(run: &RunLocation) -> ScriptLedger {
    serde_json::from_slice(&std::fs::read(run.dir.join("ledger.json")).unwrap()).unwrap()
}

fn decisions(ledger: &ScriptLedger) -> Vec<CandidateDecision> {
    ledger.candidates.iter().map(|c| c.decision).collect()
}

/// Ledger and candidate directories after one better and one worse proposal.
fn assert_better_worse_ledger(run: &RunLocation, report: &LoopReport) {
    let ledger = ledger(run);
    assert_eq!(
        decisions(&ledger),
        [
            CandidateDecision::Baseline,
            CandidateDecision::Accept,
            CandidateDecision::Reject
        ]
    );
    assert!(ledger.candidates[1].promoted && !ledger.candidates[2].promoted);
    assert_eq!(
        ledger.candidates[1].parent.as_deref(),
        Some(report.baseline.as_str())
    );
    assert_eq!(ledger.candidates[2].runs[1].metric, Some(0.72));
    assert_eq!(ledger.stop, Some(StopReason::MaxEpochs));
    assert_eq!(ledger.incumbents.len(), 2);
    let promoted = run.dir.join("candidates").join(&report.incumbent);
    assert_eq!(
        std::fs::read_to_string(promoted.join("run.sh")).unwrap(),
        loss("0.3")
    );
    assert!(promoted.join("data.txt").exists());
}

#[test]
fn promotes_a_better_script_and_rejects_a_worse_one() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let port = Scripted::new(vec![reply(&loss("0.3")), reply(&loss("0.7"))]);
    let mut trace = JsonlTrace::create(run.dir.join("events.jsonl"), ManualClock::new()).unwrap();
    let report = run_script_loop(
        &s,
        &supervisor(port.clone()),
        &policy(1, 2),
        &run,
        &mut trace,
    )
    .unwrap();
    assert_eq!(report.reason, StopReason::MaxEpochs);
    assert_eq!(
        (report.promotions, report.rejected, report.epochs),
        (1, 1, 1)
    );
    assert!((report.baseline_metric - 0.515).abs() < 1e-12);
    assert!((report.incumbent_metric - 0.315).abs() < 1e-12);
    assert_eq!(report.tokens, 300);
    assert_eq!(report.ledger, run.dir.join("ledger.json"));

    let text = std::fs::read_to_string(run.dir.join("events.jsonl")).unwrap();
    let records: Vec<TraceRecord> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let kinds: Vec<String> = records
        .iter()
        .map(|r| {
            serde_json::to_value(&r.event).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    let expected = [
        "run_started",
        "executed",
        "executed",
        "measured",
        "epoch_started",
        "observed",
        "ideated",
        "executed",
        "executed",
        "measured",
        "scored",
        "ideated",
        "executed",
        "executed",
        "measured",
        "scored",
        "improved",
        "epoch_finished",
        "run_finished",
    ];
    assert_eq!(kinds, expected);
    assert!(records.iter().enumerate().all(|(i, r)| r.seq == i as u64));
    let LoopEvent::Scored {
        decision, estimate, ..
    } = &records[10].event
    else {
        panic!("expected scored")
    };
    assert_eq!(*decision, crate::ScoreDecision::Accept);
    assert!((estimate - 0.2).abs() < 1e-12);
    let LoopEvent::Improved { from, to, .. } = &records[16].event else {
        panic!("expected improved")
    };
    assert_eq!((from, to), (&report.baseline, &report.incumbent));
    let LoopEvent::Ideated { candidate, .. } = &records[6].event else {
        panic!("expected ideated")
    };
    assert_eq!(candidate.as_ref(), Some(&report.incumbent));

    assert_better_worse_ledger(&run, &report);
    // Both proposals saw the baseline; the prompt carries the incumbent source.
    let seen = port.seen.lock().unwrap();
    assert!(seen[0].0.contains("METRIC loss=0.5"));
    assert_ne!(seen[0].1, seen[1].1);
}

#[test]
fn builds_the_next_epoch_on_the_promoted_incumbent() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let port = Scripted::new(vec![
        reply(&loss("0.4")),
        reply(&loss("0.3")),
        reply(&loss("0.2")),
        reply(&loss("0.25")),
    ]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(
        &s,
        &supervisor(port.clone()),
        &policy(2, 2),
        &run,
        &mut trace,
    )
    .unwrap();
    // Epoch 1 accepts both and promotes the better; epoch 2 builds on it.
    assert!((report.incumbent_metric - 0.215).abs() < 1e-12);
    assert_eq!(report.promotions, 2);
    let ledger = ledger(&run);
    assert!(!ledger.candidates[1].promoted && ledger.candidates[2].promoted);
    assert_eq!(ledger.candidates[3].parent, ledger.candidates[2].id);
    assert!(port.seen.lock().unwrap()[2].0.contains("METRIC loss=0.3"));
}

#[test]
fn stops_when_the_goal_is_reached() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = subject(dir.path(), &loss("0.5"));
    s.metric.target = Some(0.35);
    let run = location(dir.path());
    let port = Scripted::new(vec![reply(&loss("0.3"))]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(&s, &supervisor(port), &policy(5, 1), &run, &mut trace).unwrap();
    assert_eq!((report.reason, report.epochs), (StopReason::GoalReached, 1));
}

#[test]
fn stops_before_any_epoch_when_the_baseline_meets_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = subject(dir.path(), &loss("0.5"));
    s.metric.target = Some(0.6);
    let run = location(dir.path());
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(
        &s,
        &supervisor(Scripted::new(vec![])),
        &policy(5, 1),
        &run,
        &mut trace,
    )
    .unwrap();
    assert_eq!((report.reason, report.epochs), (StopReason::GoalReached, 0));
    assert_eq!(kinds(&trace).last().unwrap(), "run_finished");
}

#[test]
fn stops_as_stalled_after_patience_epochs_without_promotion() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let port = Scripted::new(vec![reply(&loss("0.7")), reply(&loss("0.8"))]);
    let mut trace = VecTrace::new(ManualClock::new());
    let p = LoopPolicy {
        patience: 2,
        ..policy(5, 1)
    };
    let report = run_script_loop(&s, &supervisor(port), &p, &run, &mut trace).unwrap();
    assert_eq!((report.reason, report.epochs), (StopReason::Stalled, 2));
    assert_eq!(report.promotions, 0);
}

#[test]
fn stops_when_the_token_budget_is_spent() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let port = Scripted::new(vec![reply(&loss("0.3")), reply(&loss("0.2"))]);
    let mut trace = VecTrace::new(ManualClock::new());
    let p = LoopPolicy {
        token_budget: 100,
        ..policy(5, 2)
    };
    let report = run_script_loop(&s, &supervisor(port.clone()), &p, &run, &mut trace).unwrap();
    assert_eq!((report.reason, report.tokens), (StopReason::Budget, 150));
    assert_eq!(port.seen.lock().unwrap().len(), 1);
    assert_eq!(
        kinds(&trace)
            .iter()
            .filter(|k| *k == "epoch_finished")
            .count(),
        1
    );
}

#[test]
fn records_invalid_and_duplicate_proposals_and_continues() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let escape = serde_json::json!({
        "rationale": "r", "hypothesis": "h",
        "files": [{"path": "../escape.sh", "content": "rm -rf /"}]
    });
    let port = Scripted::new(vec![
        escape.to_string(),
        "{not json".into(),
        reply(&loss("0.5")),
        reply(&loss("0.3")),
    ]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(&s, &supervisor(port), &policy(1, 4), &run, &mut trace).unwrap();
    let ledger = ledger(&run);
    assert_eq!(
        decisions(&ledger),
        [
            CandidateDecision::Baseline,
            CandidateDecision::Invalid,
            CandidateDecision::Invalid,
            CandidateDecision::Duplicate,
            CandidateDecision::Accept
        ]
    );
    assert_eq!(
        ledger.candidates[1].note.as_deref(),
        Some("proposal edits a path outside the editable set")
    );
    assert_eq!(
        ledger.candidates[2].note.as_deref(),
        Some("supervisor reply is not a well-formed proposal")
    );
    assert!(!dir.path().join("escape.sh").exists());
    let ideated: Vec<Option<String>> = trace
        .records()
        .iter()
        .filter_map(|r| match &r.event {
            LoopEvent::Ideated { candidate, .. } => Some(candidate.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(ideated[..2], [None, None]);
    assert_eq!(ideated[2], ledger.candidates[3].id);
    assert_eq!(ideated[3], ledger.candidates[4].id);
    assert_eq!((report.promotions, report.rejected), (1, 3));
}

#[test]
fn rejects_a_candidate_that_fails_any_seed() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let flaky = "if [ \"$TINYRSI_SEED\" = 2 ]; then echo crash >&2; exit 1; fi\n\
                 echo \"METRIC loss=0.1$TINYRSI_SEED\"\n";
    let port = Scripted::new(vec![reply(flaky)]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(&s, &supervisor(port), &policy(1, 1), &run, &mut trace).unwrap();
    assert_eq!(report.promotions, 0);
    let ledger = ledger(&run);
    let row = &ledger.candidates[1];
    assert_eq!(row.decision, CandidateDecision::Reject);
    assert_eq!(row.runs[1].stderr_tail, "crash\n");
    assert!((row.estimate.unwrap() - 0.4).abs() < 1e-12);
}

#[test]
fn fails_with_a_terminal_event_when_the_baseline_fails() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), "exit 1\n");
    let run = location(dir.path());
    let mut trace = VecTrace::new(ManualClock::new());
    let error = run_script_loop(
        &s,
        &supervisor(Scripted::new(vec![])),
        &policy(1, 1),
        &run,
        &mut trace,
    )
    .unwrap_err();
    assert_eq!(error, Error::ScriptBaselineFailed);
    let last = &trace.records().last().unwrap().event;
    assert!(matches!(
        last,
        LoopEvent::RunFinished {
            reason: StopReason::Error,
            incumbent_metric: None,
            ..
        }
    ));
    assert_eq!(ledger(&run).stop, Some(StopReason::Error));
}

#[test]
fn ends_with_an_error_event_when_the_model_call_fails() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let mut trace = VecTrace::new(ManualClock::new());
    let error = run_script_loop(
        &s,
        &supervisor(Scripted::new(vec![])),
        &policy(1, 1),
        &run,
        &mut trace,
    )
    .unwrap_err();
    assert_eq!(error, Error::HostFailed);
    assert_eq!(kinds(&trace).last().unwrap(), "run_finished");
    assert_eq!(ledger(&run).stop, Some(StopReason::Error));
}

#[test]
fn rejects_invalid_policies_and_locations_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let sup = supervisor(Scripted::new(vec![]));
    let mut bad_eval = policy(1, 1);
    bad_eval.eval.standard_error_multiplier = 1.0;
    let cases = [
        policy(0, 1),
        policy(1, 0),
        LoopPolicy {
            patience: 0,
            ..policy(1, 1)
        },
        LoopPolicy {
            token_budget: 0,
            ..policy(1, 1)
        },
        bad_eval,
    ];
    let run = location(dir.path());
    for p in &cases {
        let mut trace = VecTrace::new(ManualClock::new());
        let error = run_script_loop(&s, &sup, p, &run, &mut trace).unwrap_err();
        assert_eq!(error, Error::InvalidScriptSubject);
        assert_eq!(trace.records().len(), 0);
    }
    let bad_id = RunLocation {
        id: "a\nb".into(),
        dir: run.dir.clone(),
    };
    let mut trace = VecTrace::new(ManualClock::new());
    assert_eq!(
        run_script_loop(&s, &sup, &policy(1, 1), &bad_id, &mut trace).unwrap_err(),
        Error::InvalidScriptSubject
    );
    assert!(!run.dir.exists());
}

#[test]
fn propagates_trace_failures() {
    struct Broken;
    impl crate::TraceSink for Broken {
        fn record(&mut self, _: LoopEvent) -> tinyrsi_trace::Result<()> {
            Err(crate::TraceError::Io)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let error = run_script_loop(
        &s,
        &supervisor(Scripted::new(vec![])),
        &policy(1, 1),
        &location(dir.path()),
        &mut Broken,
    )
    .unwrap_err();
    assert_eq!(error, Error::TraceIo);
}

#[test]
fn atomic_ledger_writes_fail_cleanly_without_a_directory() {
    assert_eq!(
        write_atomic(Path::new("/nonexistent/dir/ledger.json"), &1),
        Err(Error::ArchiveIo)
    );
    assert_eq!(write_atomic(Path::new(""), &1), Err(Error::ArchiveIo));
}

/// A scorer that reads the sources instead of the metric: a script whose
/// text mentions `clear` scores high. It spends `tokens` per assessment and
/// is eligible only when `eligible` is set, like a judge.
fn source_judge(tokens: u64, eligible: bool) -> tinyrsi_score::ScriptedScorer<ScriptCandidate> {
    tinyrsi_score::ScriptedScorer::new(move |c: &ScriptCandidate, _, seeds| {
        let clear = c.sources.values().any(|s| s.contains("clear"));
        let qualities: Vec<f64> = seeds
            .iter()
            .enumerate()
            .map(|(i, _)| if clear { 0.9 } else { 0.1 } + 0.001 * f64::from(u8::try_from(i).unwrap()))
            .collect();
        Ok(tinyrsi_score::Assessment {
            eligible: eligible.then(|| qualities.clone()),
            qualities,
            metric: None,
            rationale: Some("reads the sources".into()),
            tokens,
        })
    })
}

#[test]
fn a_custom_scorer_decides_on_what_it_reads_and_its_tokens_count() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    // A worse metric, but a script the source judge prefers.
    let clearer = format!("# clear\n{}", loss("0.7"));
    let port = Scripted::new(vec![reply(&clearer)]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop_with(
        &s,
        &supervisor(port),
        &source_judge(25, true),
        &policy(1, 1),
        &run,
        &mut trace,
    )
    .unwrap();
    let ledger = ledger(&run);
    assert_eq!(
        decisions(&ledger),
        [CandidateDecision::Baseline, CandidateDecision::Accept]
    );
    assert_eq!(report.promotions, 1);
    // Supervisor 150 + baseline assessment 25 + candidate assessment 25.
    assert_eq!(ledger.tokens, 200);
    // A blend of judge evidence reports its interval in quality units.
    assert!((ledger.candidates[1].estimate.unwrap() - 0.8).abs() < 1e-9);
}

#[test]
fn an_uncalibrated_scorer_never_promotes_on_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let clearer = format!("# clear\n{}", loss("0.7"));
    let port = Scripted::new(vec![reply(&clearer)]);
    let report = run_script_loop_with(
        &s,
        &supervisor(port),
        &source_judge(0, false),
        &policy(1, 1),
        &run,
        &mut VecTrace::new(ManualClock::new()),
    )
    .unwrap();
    assert_eq!(report.promotions, 0);
    assert_eq!(
        decisions(&ledger(&run)),
        [CandidateDecision::Baseline, CandidateDecision::Inconclusive]
    );
}

#[test]
fn a_failing_scorer_ends_the_run_with_its_error() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let failing: tinyrsi_score::ScriptedScorer<ScriptCandidate> =
        tinyrsi_score::ScriptedScorer::new(|_, _, _| Err(tinyrsi_score::Error::JudgeMalformed));
    let mut trace = VecTrace::new(ManualClock::new());
    let error = run_script_loop_with(
        &s,
        &supervisor(Scripted::new(vec![])),
        &failing,
        &policy(1, 1),
        &run,
        &mut trace,
    )
    .unwrap_err();
    assert_eq!(error, Error::JudgeMalformed);
    assert_eq!(
        kinds(&trace).last().map(String::as_str),
        Some("run_finished")
    );
}
