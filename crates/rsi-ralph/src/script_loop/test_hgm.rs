//! `Search::Hgm`: clade statistics, Thompson selection and the loop.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::hgm::{Clade, clades, cooling, draw_seed, select};
use super::test::{Scripted, loss, reply, subject, supervisor};
use super::*;
use crate::{ManualClock, VecTrace};
use std::collections::BTreeMap;

fn run_row(id: &str, parent: Option<&str>, metrics: &[Option<f64>]) -> LedgerCandidate {
    LedgerCandidate {
        epoch: u64::from(parent.is_some()),
        proposal_id: parent.map(|_| "p".into()),
        id: Some(id.into()),
        parent: parent.map(str::to_owned),
        rationale: String::new(),
        hypothesis: String::new(),
        paths: vec![],
        runs: metrics
            .iter()
            .enumerate()
            .map(|(i, m)| SeedRun {
                seed: u64::try_from(i).unwrap(),
                metric: *m,
                error: None,
                duration_ms: 0,
                stderr_tail: String::new(),
                diagnostics: vec![],
            })
            .collect(),
        mean: None,
        decision: if parent.is_some() {
            CandidateDecision::Reject
        } else {
            CandidateDecision::Baseline
        },
        estimate: None,
        lower: None,
        upper: None,
        units: IntervalUnits::Metric,
        promoted: false,
        note: None,
        approach: None,
        differs_from_previous: None,
        temperature: None,
    }
}

fn tree(target: Option<f64>, rows: Vec<LedgerCandidate>) -> ScriptLedger {
    ScriptLedger {
        run_id: "r".into(),
        subject: "s".into(),
        metric: MetricSpec {
            name: "colors".into(),
            direction: crate::MetricDirection::Minimize,
            target,
        },
        seeds: vec![0, 1],
        tokens: 0,
        stop: None,
        incumbents: vec![],
        candidates: rows,
        restarts: vec![],
    }
}

fn close(a: Clade, s: f64, f: f64) -> bool {
    (a.successes - s).abs() < 1e-9 && (a.failures - f).abs() < 1e-9
}

#[test]
fn pools_each_seeds_win_against_its_parent_into_every_ancestor() {
    let ledger = tree(
        Some(0.0),
        vec![
            run_row("root", None, &[Some(10.0), Some(10.0)]),
            // a beats root on both seeds.
            run_row("a", Some("root"), &[Some(5.0), Some(5.0)]),
            // b beats a on seed 0 and fails seed 1.
            run_row("b", Some("a"), &[Some(0.0), None]),
            // c is worse than root on seed 0 and ties on seed 1.
            run_row("c", Some("root"), &[Some(20.0), Some(10.0)]),
        ],
    );
    let c = clades(&ledger);
    assert!(close(c["b"], 1.0, 1.0));
    assert!(close(c["a"], 3.0, 1.0));
    assert!(close(c["c"], 0.5, 1.5));
    // The root has no outcome of its own: only its descendants'.
    assert!(close(c["root"], 3.5, 2.5));
}

fn at_epoch(mut row: LedgerCandidate, epoch: u64) -> LedgerCandidate {
    row.epoch = epoch;
    row
}

fn champions(ledger: &mut ScriptLedger, chain: &[(u64, &str)]) {
    ledger.incumbents = chain
        .iter()
        .map(|(epoch, id)| IncumbentEntry {
            epoch: *epoch,
            id: (*id).into(),
            metric: 0.0,
        })
        .collect();
}

#[test]
fn near_target_candidates_still_differ_by_whether_they_beat_the_champion() {
    let mut ledger = tree(
        Some(0.06),
        vec![
            run_row("root", None, &[Some(0.644), Some(0.644)]),
            run_row("good", Some("root"), &[Some(0.070), Some(0.070)]),
            at_epoch(
                run_row("better", Some("good"), &[Some(0.065), Some(0.066)]),
                2,
            ),
            at_epoch(
                run_row("stalled", Some("good"), &[Some(0.071), Some(0.072)]),
                2,
            ),
        ],
    );
    champions(&mut ledger, &[(0, "root"), (1, "good")]);
    let c = clades(&ledger);
    assert!(close(c["better"], 2.0, 0.0));
    assert!(close(c["stalled"], 0.0, 2.0));
}

#[test]
fn improving_a_weak_branch_does_not_count_until_it_beats_the_champion() {
    // Graph colouring: the champion has 13 colours, a weak branch 15.
    let mut ledger = tree(
        Some(6.0),
        vec![
            run_row("root", None, &[Some(17.0), Some(17.0)]),
            run_row("champ", Some("root"), &[Some(13.0), Some(13.0)]),
            run_row("weak", Some("root"), &[Some(15.0), Some(15.0)]),
            at_epoch(run_row("weak+", Some("weak"), &[Some(14.0), Some(14.0)]), 2),
            at_epoch(run_row("leap", Some("weak"), &[Some(9.0), Some(13.0)]), 2),
        ],
    );
    champions(&mut ledger, &[(0, "root"), (1, "champ")]);
    let c = clades(&ledger);
    // 15 -> 14 beat its parent but not the 13-colour champion.
    assert!(close(c["weak+"], 0.0, 2.0));
    // 9 beats 13 on seed 0 and ties on seed 1.
    assert!(close(c["leap"], 1.5, 0.5));
    // The weak node beat the root (champion before epoch 1) on both seeds.
    assert!(close(c["weak"], 2.0 + 1.5, 2.0 + 0.5));
}

#[test]
fn duplicates_and_invalid_rows_add_no_node() {
    let mut dup = run_row("a", Some("root"), &[Some(0.0), Some(0.0)]);
    dup.decision = CandidateDecision::Duplicate;
    let ledger = tree(
        Some(0.0),
        vec![run_row("root", None, &[Some(10.0), Some(10.0)]), dup],
    );
    let c = clades(&ledger);
    assert!(!c.contains_key("a"));
    assert!(close(c["root"], 0.0, 0.0));
}

#[test]
fn thompson_selection_favours_productive_clades_and_is_deterministic() {
    let mut clades = BTreeMap::new();
    clades.insert(
        "good".to_owned(),
        Clade {
            successes: 40.0,
            failures: 2.0,
        },
    );
    clades.insert(
        "poor".to_owned(),
        Clade {
            successes: 2.0,
            failures: 40.0,
        },
    );
    let picks: Vec<&str> = (0..50)
        .map(|k| select(&clades, &["poor", "good"], draw_seed("run", 1, k), 1.0).unwrap())
        .collect();
    assert!(picks.iter().filter(|p| **p == "good").count() >= 48);
    assert_eq!(
        select(&clades, &["poor", "good"], 7, 1.0),
        select(&clades, &["poor", "good"], 7, 1.0)
    );
    assert_eq!(select(&clades, &[], 7, 1.0), None);
    // Equal, uninformed clades are both chosen sometimes.
    let flat: BTreeMap<String, Clade> = BTreeMap::new();
    let picks: Vec<&str> = (0..50)
        .map(|k| select(&flat, &["x", "y"], draw_seed("run", 2, k), 1.0).unwrap())
        .collect();
    assert!(picks.contains(&"x") && picks.contains(&"y"));
}

#[test]
fn cooling_rises_from_one_to_the_epoch_count() {
    assert!((cooling(1, 10) - 1.0).abs() < 1e-12);
    assert!((cooling(6, 10) - 2.0).abs() < 1e-12);
    assert!((cooling(10, 10) - 10.0).abs() < 1e-12);
    assert!((cooling(12, 10) - 10.0).abs() < 1e-12);
    assert!((cooling(1, 0) - 1.0).abs() < 1e-12);
}

#[test]
fn cooling_narrows_draws_towards_the_better_clade() {
    // Close records: 6 of 10 against 4 of 10.
    let mut clades = BTreeMap::new();
    clades.insert(
        "a".to_owned(),
        Clade {
            successes: 6.0,
            failures: 4.0,
        },
    );
    clades.insert(
        "b".to_owned(),
        Clade {
            successes: 4.0,
            failures: 6.0,
        },
    );
    let wins = |tau: f64| {
        (0..200)
            .filter(|&k| select(&clades, &["b", "a"], draw_seed("run", 3, k), tau) == Some("a"))
            .count()
    };
    let (warm, cold) = (wins(1.0), wins(10.0));
    assert!(warm < 180, "warm draws still explore: {warm}");
    assert!(cold > warm, "cooled draws exploit more: {cold} vs {warm}");
    assert!(
        cold >= 190,
        "cold draws almost always pick the better clade: {cold}"
    );
}

#[test]
fn draw_seeds_differ_by_run_epoch_and_slot() {
    let seeds = [
        draw_seed("a", 1, 0),
        draw_seed("a", 1, 1),
        draw_seed("a", 2, 0),
        draw_seed("b", 1, 0),
    ];
    for (i, x) in seeds.iter().enumerate() {
        assert!(seeds[i + 1..].iter().all(|y| y != x));
    }
}

#[test]
fn hgm_expands_from_non_champion_nodes_and_promotes_only_on_a_win() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = RunLocation {
        id: "run-1".into(),
        dir: dir.path().join("run"),
    };
    let mut replies = vec![];
    for prefix in [
        "0.4", "0.45", "0.43", "0.44", "0.41", "0.42", "0.39", "0.38",
    ] {
        replies.push(reply(&loss(prefix)));
    }
    let port = Scripted::new(replies);
    let policy = LoopPolicy {
        max_epochs: 4,
        proposals_per_epoch: 2,
        patience: 1,
        search: Search::Hgm,
        ..LoopPolicy::default()
    };
    let mut trace = VecTrace::new(ManualClock::new());
    let report = run_script_loop(&s, &supervisor(port.clone()), &policy, &run, &mut trace).unwrap();
    let ledger: ScriptLedger =
        serde_json::from_slice(&std::fs::read(run.dir.join("ledger.json")).unwrap()).unwrap();
    // Patience 1 never stopped the run.
    assert_eq!((report.reason, report.epochs), (StopReason::MaxEpochs, 4));
    // Every candidate's parent is a measured node of the tree.
    let ids: Vec<&str> = ledger
        .candidates
        .iter()
        .filter_map(|c| c.id.as_deref())
        .collect();
    assert!(
        ledger.candidates[1..]
            .iter()
            .all(|c| ids.contains(&c.parent.as_deref().unwrap()))
    );
    // The champion chain only ever improves.
    assert!(
        ledger
            .incumbents
            .windows(2)
            .all(|w| w[1].metric < w[0].metric)
    );
    assert!((report.incumbent_metric - ledger.incumbents.last().unwrap().metric).abs() < 1e-12);
    // A proposal that edits a non-champion node says so in its prompt.
    let champions: Vec<&str> = ledger.incumbents.iter().map(|i| i.id.as_str()).collect();
    let seen = port.seen.lock().unwrap();
    let mut off_champion = 0;
    for (row, (prompt, _)) in ledger.candidates[1..].iter().zip(seen.iter()) {
        let parent = row.parent.as_deref().unwrap();
        let champion_then = ledger
            .incumbents
            .iter()
            .rev()
            .find(|i| i.epoch < row.epoch)
            .unwrap();
        assert_eq!(
            prompt.contains("# Starting point"),
            parent != champion_then.id,
            "epoch {} parent {parent}",
            row.epoch
        );
        assert!(champions.contains(&champion_then.id.as_str()));
        off_champion += usize::from(parent != champion_then.id);
    }
    // The search did edit nodes other than the champion.
    assert!(off_champion > 0);
}
