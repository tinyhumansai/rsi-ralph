//! `Search::Restart`: restarting from an earlier candidate when the parent
//! stalls, while the champion only moves on a paired win.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::test::{Scripted, loss, reply, subject, supervisor};
use super::*;
use crate::{ManualClock, VecTrace};
use std::path::Path;

fn location(dir: &Path) -> RunLocation {
    RunLocation {
        id: "run-1".into(),
        dir: dir.join("run"),
    }
}

fn restart_policy(max_epochs: u64, proposals_per_epoch: u32, patience: u32) -> LoopPolicy {
    LoopPolicy {
        max_epochs,
        proposals_per_epoch,
        patience,
        search: Search::Restart,
        ..LoopPolicy::default()
    }
}

fn ledger(run: &RunLocation) -> ScriptLedger {
    serde_json::from_slice(&std::fs::read(run.dir.join("ledger.json")).unwrap()).unwrap()
}

/// The ledger row of the candidate whose script prints `prefix`.
fn row<'a>(ledger: &'a ScriptLedger, prefix: &str) -> &'a LedgerCandidate {
    let id = content_id(&[("run.sh".into(), loss(prefix))].into_iter().collect());
    ledger
        .candidates
        .iter()
        .find(|c| c.id.as_deref() == Some(&id))
        .unwrap()
}

#[test]
fn hill_search_is_the_default_and_writes_no_restarts() {
    assert_eq!(LoopPolicy::default().search, Search::Hill);
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let port = Scripted::new(vec![reply(&loss("0.7"))]);
    let policy = LoopPolicy {
        max_epochs: 3,
        proposals_per_epoch: 1,
        patience: 1,
        ..LoopPolicy::default()
    };
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(&s, &supervisor(port), &policy, &run, &mut trace).unwrap();
    assert_eq!((report.reason, report.epochs), (StopReason::Stalled, 1));
    let text = std::fs::read_to_string(run.dir.join("ledger.json")).unwrap();
    assert!(!text.contains("restarts"));
}

#[test]
fn restarts_from_the_best_distinct_candidate_and_promotes_only_on_a_win() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let port = Scripted::new(vec![
        // Epoch 1: both beat the baseline; 0.4 becomes champion.
        reply(&loss("0.4")),
        reply(&loss("0.45")),
        // Epoch 2: both worse; the champion stalls and the loop restarts
        // from 0.45, the best candidate that was never a parent.
        reply(&loss("0.6")),
        reply(&loss("0.7")),
        // Epoch 3: both beat the 0.45 parent, neither the 0.4 champion.
        reply(&loss("0.42")),
        reply(&loss("0.44")),
        // Epoch 4: 0.3 beats the 0.42 parent and the champion.
        reply(&loss("0.3")),
        reply(&loss("0.35")),
    ]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(
        &s,
        &supervisor(port.clone()),
        &restart_policy(4, 2, 1),
        &run,
        &mut trace,
    )
    .unwrap();
    let ledger = ledger(&run);
    let (champion, restarted) = (row(&ledger, "0.4"), row(&ledger, "0.45"));
    assert_eq!(
        ledger.restarts,
        [RestartEntry {
            epoch: 2,
            from: champion.id.clone().unwrap(),
            to: restarted.id.clone().unwrap(),
            metric: restarted.mean.unwrap(),
        }]
    );
    // Epoch 3 improved the branch but not the champion.
    let branch = row(&ledger, "0.42");
    assert_eq!(branch.parent, restarted.id);
    assert!(!branch.promoted);
    assert_eq!(branch.note.as_deref(), Some(BRANCH_NOTE));
    assert_eq!(ledger.incumbents[1].id, champion.id.clone().unwrap());
    // Epoch 4 built on the branch's new parent and beat the champion.
    let winner = row(&ledger, "0.3");
    assert_eq!(winner.parent, branch.id);
    assert!(winner.promoted);
    assert_eq!(ledger.incumbents.len(), 3);
    assert_eq!(
        (report.reason, report.promotions),
        (StopReason::MaxEpochs, 2)
    );
    assert_eq!(Some(&report.incumbent), winner.id.as_ref());
    assert!((report.incumbent_metric - 0.315).abs() < 1e-12);
    // The branch's prompts edit the restarted candidate and say so.
    let seen = port.seen.lock().unwrap();
    assert!(seen[4].0.contains("# Restart branch") && seen[4].0.contains("METRIC loss=0.45"));
    assert!(!seen[2].0.contains("# Restart branch"));
}

#[test]
fn keeps_the_champion_while_a_branch_improves_without_beating_it() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    let port = Scripted::new(vec![
        reply(&loss("0.4")),
        reply(&loss("0.45")),
        reply(&loss("0.6")),
        reply(&loss("0.7")),
        reply(&loss("0.42")),
        reply(&loss("0.44")),
    ]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(
        &s,
        &supervisor(port),
        &restart_policy(3, 2, 1),
        &run,
        &mut trace,
    )
    .unwrap();
    let ledger = ledger(&run);
    assert_eq!(Some(&report.incumbent), row(&ledger, "0.4").id.as_ref());
    assert!((report.incumbent_metric - 0.415).abs() < 1e-12);
    assert_eq!(report.promotions, 1);
    let LoopEvent::RunFinished { incumbent, .. } = &trace.records().last().unwrap().event else {
        unreachable!("the trace ends with run_finished")
    };
    assert_eq!(incumbent, &report.incumbent);
}

#[test]
fn a_stalled_branch_returns_to_the_champion_and_lookalikes_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = location(dir.path());
    // A different script with the same per-seed metrics as the 0.7 parent.
    let lookalike = format!("{}# same numbers\n", loss("0.7"));
    let port = Scripted::new(vec![
        // Epoch 1 on the champion: worse, so the loop restarts from it.
        reply(&loss("0.7")),
        // Epoch 2 on the 0.7 branch: no better, so it returns to the champion.
        reply(&lookalike),
        // Epoch 3 on the champion again: a repeat. The only candidates left
        // are the 0.7 parent and its lookalike, so the run stalls.
        reply(&loss("0.7")),
    ]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(
        &s,
        &supervisor(port.clone()),
        &restart_policy(5, 1, 1),
        &run,
        &mut trace,
    )
    .unwrap();
    let ledger = ledger(&run);
    assert_eq!((report.reason, report.epochs), (StopReason::Stalled, 3));
    let (baseline, branch) = (
        report.baseline.clone(),
        row(&ledger, "0.7").id.clone().unwrap(),
    );
    let moves: Vec<(u64, &str, &str)> = ledger
        .restarts
        .iter()
        .map(|r| (r.epoch, r.from.as_str(), r.to.as_str()))
        .collect();
    assert_eq!(
        moves,
        [
            (1, baseline.as_str(), branch.as_str()),
            (2, branch.as_str(), baseline.as_str())
        ]
    );
    assert!((ledger.restarts[1].metric - 0.515).abs() < 1e-12);
    // Back on the champion, the prompt is the plain one again.
    let seen = port.seen.lock().unwrap();
    assert!(seen[1].0.contains("# Restart branch"));
    assert!(!seen[2].0.contains("# Restart branch"));
    // The champion never moved off the baseline.
    assert_eq!(report.promotions, 0);
    assert!((report.incumbent_metric - 0.515).abs() < 1e-12);
}

#[test]
fn compares_metrics_within_rounding() {
    assert!(same_metrics(&[0.1 + 0.2, 1.0], &[0.3, 1.0]));
    assert!(!same_metrics(&[0.3, 1.0], &[0.3, 1.01]));
    assert!(!same_metrics(&[0.3], &[0.3, 1.0]));
}

#[test]
fn pins_the_restart_entry_wire_form() {
    let entry = RestartEntry {
        epoch: 2,
        from: "aaa".into(),
        to: "bbb".into(),
        metric: 1.5,
    };
    assert_eq!(
        serde_json::to_value(&entry).unwrap(),
        serde_json::json!({"epoch": 2, "from": "aaa", "to": "bbb", "metric": 1.5})
    );
}
