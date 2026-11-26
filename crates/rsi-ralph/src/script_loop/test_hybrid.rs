//! `Search::Hybrid`: hill climbing with HGM detours on a stall.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::test::{Scripted, loss, reply, subject, supervisor};
use super::*;
use crate::{ManualClock, VecTrace};
use std::path::Path;

fn hybrid(max_epochs: u64, proposals_per_epoch: u32, patience: u32) -> LoopPolicy {
    LoopPolicy {
        max_epochs,
        proposals_per_epoch,
        patience,
        search: Search::Hybrid,
        ..LoopPolicy::default()
    }
}

fn run_in(dir: &Path) -> RunLocation {
    RunLocation {
        id: "run-1".into(),
        dir: dir.join("run"),
    }
}

fn ledger(run: &RunLocation) -> ScriptLedger {
    serde_json::from_slice(&std::fs::read(run.dir.join("ledger.json")).unwrap()).unwrap()
}

fn id_of(ledger: &ScriptLedger, prefix: &str) -> String {
    let id = content_id(&[("run.sh".into(), loss(prefix))].into_iter().collect());
    assert!(
        ledger
            .candidates
            .iter()
            .any(|c| c.id.as_deref() == Some(&id))
    );
    id
}

#[test]
fn detours_off_the_champion_chain_on_a_stall_and_promotes_only_on_a_win() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = run_in(dir.path());
    let port = Scripted::new(vec![
        // Epoch 1: 0.4 becomes champion, 0.45 is a runner-up.
        reply(&loss("0.4")),
        reply(&loss("0.45")),
        // Epoch 2: both worse; the champion stalls. The only node off the
        // champion chain that beats the baseline is 0.45.
        reply(&loss("0.6")),
        reply(&loss("0.7")),
        // Epoch 3: the detour improves on 0.45 but not on the champion.
        reply(&loss("0.42")),
        reply(&loss("0.44")),
        // Epoch 4: the detour beats the champion.
        reply(&loss("0.3")),
        reply(&loss("0.35")),
    ]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(
        &s,
        &supervisor(port.clone()),
        &hybrid(4, 2, 1),
        &run,
        &mut trace,
    )
    .unwrap();
    let ledger = ledger(&run);
    assert_eq!(ledger.restarts.len(), 1);
    assert_eq!(ledger.restarts[0].from, id_of(&ledger, "0.4"));
    assert_eq!(ledger.restarts[0].to, id_of(&ledger, "0.45"));
    assert_eq!(report.promotions, 2);
    assert_eq!(report.incumbent, id_of(&ledger, "0.3"));
    // Detour prompts name the starting point.
    let seen = port.seen.lock().unwrap();
    assert!(seen[4].0.contains("# Starting point") && seen[4].0.contains("METRIC loss=0.45"));
    assert!(!seen[2].0.contains("# Starting point"));
}

#[test]
fn keeps_hill_climbing_when_there_is_nowhere_to_detour() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = run_in(dir.path());
    // Every proposal is worse than the baseline, so no node qualifies.
    let port = Scripted::new(vec![
        reply(&loss("0.7")),
        reply(&loss("0.8")),
        reply(&loss("0.9")),
    ]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report =
        run_script_loop(&s, &supervisor(port), &hybrid(3, 1, 1), &run, &mut trace).unwrap();
    assert_eq!((report.reason, report.epochs), (StopReason::MaxEpochs, 3));
    assert!(ledger(&run).restarts.is_empty());
}

#[test]
fn a_stalled_detour_returns_to_the_champion() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = run_in(dir.path());
    let port = Scripted::new(vec![
        reply(&loss("0.4")),
        reply(&loss("0.45")),
        // Epoch 2: champion stalls; detour to 0.45.
        reply(&loss("0.6")),
        reply(&loss("0.7")),
        // Epoch 3: the detour gets nowhere; back to the champion.
        reply(&loss("0.8")),
        reply(&loss("0.9")),
        // Epoch 4: on the champion again.
        reply(&loss("0.95")),
        reply(&loss("0.96")),
    ]);
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(
        &s,
        &supervisor(port.clone()),
        &hybrid(4, 2, 1),
        &run,
        &mut trace,
    )
    .unwrap();
    let ledger = ledger(&run);
    let champion = id_of(&ledger, "0.4");
    assert!(ledger.restarts.len() >= 2);
    assert_eq!(ledger.restarts[1].from, id_of(&ledger, "0.45"));
    assert_eq!(ledger.restarts[1].to, champion);
    assert_eq!(report.incumbent, champion);
    let seen = port.seen.lock().unwrap();
    assert!(!seen[6].0.contains("# Starting point"));
}
