//! Score history from traces and ledgers, its placing of upgrades, and its
//! text and CSV forms.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use serde_json::json;

use super::*;
use crate::{
    store::{RunRecord, RunStatus, write_json},
    upgrades::{Applied, AppliesFrom, Author, Change, Contributor, Role, Upgrade},
};

/// A trace shaped like the Erdős 779 run 10: a baseline, one finished epoch
/// with two candidates and a promotion, and an epoch still running.
fn trace() -> String {
    [
        json!({"seq":0,"elapsed_ms":0,"event":{"kind":"run_started","run_id":"r10","subject":"erdos-779","goal":{"metric":"ladder","direction":"maximize","target":1.0}}}),
        json!({"seq":1,"elapsed_ms":47,"event":{"kind":"executed","epoch":0,"candidate":"a6c8","seed":1,"metric":0.1047,"ok":true,"duration_ms":24}}),
        json!({"seq":2,"elapsed_ms":47,"event":{"kind":"measured","epoch":0,"candidate":"a6c8","mean":0.1047,"failures":0,"seeds":2}}),
        json!({"seq":3,"elapsed_ms":47,"event":{"kind":"epoch_started","epoch":1,"incumbent":"a6c8"}}),
        json!({"seq":4,"elapsed_ms":47,"event":{"kind":"observed","epoch":1,"incumbent":"a6c8","incumbent_metric":0.1047,"history":0,"rejected":0}}),
        json!({"seq":5,"elapsed_ms":2_706_998,"event":{"kind":"ideated","epoch":1,"proposal_id":"e1-chen-a","parent":"a6c8","candidate":"e1-chen-a","rationale":"chen-a changed 5 files","hypothesis":"x"}}),
        json!({"seq":6,"elapsed_ms":2_823_325,"event":{"kind":"measured","epoch":1,"candidate":"e1-chen-a","mean":0.1065,"failures":0,"seeds":2}}),
        json!({"seq":7,"elapsed_ms":3_600_000,"event":{"kind":"measured","epoch":1,"candidate":"e1-main-b","mean":0.1087,"failures":1,"seeds":2}}),
        json!({"seq":8,"elapsed_ms":3_695_536,"event":{"kind":"improved","epoch":1,"from":"a6c8","to":"8d2d","metric":0.1087}}),
        json!({"seq":9,"elapsed_ms":3_695_536,"event":{"kind":"epoch_finished","epoch":1,"incumbent":"8d2d","incumbent_metric":0.1087}}),
        json!({"seq":10,"elapsed_ms":3_695_536,"event":{"kind":"epoch_started","epoch":2,"incumbent":"8d2d"}}),
        json!({"seq":11,"elapsed_ms":3_695_536,"event":{"kind":"observed","epoch":2,"incumbent":"8d2d","incumbent_metric":0.1087,"history":2,"rejected":1}}),
    ]
    .iter()
    .map(|v| v.to_string() + "\n")
    .collect::<String>()
        + "not json\n"
}

fn ledger() -> serde_json::Value {
    json!({
        "candidates": [
            {"id": "e1-chen-a", "seat": "chen-a", "status": "scored"},
            {"id": "e1-main-b", "seat": "main-b", "status": "promoted"}
        ],
        "epochs": [{"epoch": 1, "promoted": "e1-main-b"}]
    })
}

#[test]
fn a_trace_gives_the_baseline_each_candidate_and_the_scoring_wall_time() {
    let scores = run_scores("r10", 100, &trace(), Some(&ledger()));
    assert!(scores.maximize);
    let baseline = scores.baseline.as_ref().unwrap();
    assert_eq!(
        (baseline.candidate.as_str(), baseline.mean, baseline.seeds),
        ("a6c8", Some(0.1047), 2)
    );
    assert_eq!(scores.epochs.len(), 2);
    let e1 = &scores.epochs[0];
    assert_eq!((e1.epoch, e1.incumbent.as_str()), (1, "a6c8"));
    assert_eq!(e1.incumbent_metric, Some(0.1047));
    assert_eq!(e1.finished_metric, Some(0.1087));
    assert_eq!(e1.hive_ms, Some(2_706_998 - 47));
    assert_eq!(e1.scoring_ms, Some(3_695_536 - 2_706_998));
    let seats: Vec<_> = e1
        .candidates
        .iter()
        .map(|p| (p.seat.as_deref().unwrap(), p.mean.unwrap(), p.failures, p.promoted))
        .collect();
    assert_eq!(
        seats,
        [("chen-a", 0.1065, 0, false), ("main-b", 0.1087, 1, true)]
    );
    // The running epoch has no end and no scoring time yet.
    let e2 = &scores.epochs[1];
    assert_eq!((e2.finished_metric, e2.scoring_ms, e2.hive_ms), (None, None, None));
}

#[test]
fn without_a_ledger_seats_come_from_ids_and_the_promotion_from_its_score() {
    let scores = run_scores("r10", 100, &trace(), None);
    let e1 = &scores.epochs[0];
    assert_eq!(e1.candidates[0].seat.as_deref(), Some("chen-a"));
    assert_eq!(
        e1.candidates.iter().map(|p| p.promoted).collect::<Vec<_>>(),
        [false, true]
    );
    let summary = scores.summary();
    assert_eq!(summary.start, Some(0.1047));
    assert_eq!(summary.best, Some(0.1087));
    assert_eq!(summary.end, Some(0.1087));
    assert_eq!((summary.candidates, summary.promoted), (2, 1));
}

#[test]
fn a_minimizing_run_counts_the_lowest_score_as_best() {
    let text = trace().replace("\"maximize\"", "\"minimize\"");
    let scores = run_scores("r", 0, &text, None);
    assert!(!scores.maximize);
    assert_eq!(scores.summary().best, Some(0.1065));
}

fn upgrade(id: &str, run: Option<&str>) -> Entry {
    Entry::Upgrade(Upgrade {
        id: id.into(),
        recorded_at: 0,
        title: format!("title {id}"),
        contributors: vec![Contributor {
            author: Author::Operator { name: "o".into() },
            role: Role::Authored,
            rationale: None,
        }],
        applies_from: AppliesFrom {
            run: run.map(str::to_owned),
            epoch: None,
        },
        evidence: Vec::new(),
        diagnosis: None,
        change: Change::default(),
        expected: Vec::new(),
    })
}

#[test]
fn upgrades_are_placed_between_the_run_before_and_the_run_they_apply_to() {
    let runs = vec![
        run_scores("r9", 1, &trace(), None),
        run_scores("r10", 2, &trace().replace("0.1087", "0.2"), None),
    ];
    let entries = vec![
        upgrade("u-named", Some("r10")),
        upgrade("u-carried", None),
        Entry::Applied(Applied {
            run: "r10".into(),
            recorded_at: 0,
            previous_run: Some("r9".into()),
            upgrades: vec!["u-carried".into()],
            files: Vec::new(),
            unexplained: Vec::new(),
        }),
        upgrade("u-pending", None),
        upgrade("u-elsewhere", Some("r3")),
    ];
    let placed = marks(&runs, &entries);
    assert_eq!(placed.len(), 4);
    for mark in &placed[..2] {
        assert_eq!(mark.run.as_deref(), Some("r10"));
        assert_eq!(mark.before.as_ref().unwrap().best, Some(0.1087));
        assert_eq!(mark.after.as_ref().unwrap().best, Some(0.2));
    }
    assert_eq!(placed[2].run, None);
    assert_eq!(placed[2].before.as_ref().unwrap().run, "r10");
    assert_eq!(placed[2].after, None);
    assert_eq!((placed[3].before.as_ref(), placed[3].after.as_ref()), (None, None));
    let first = marks(&runs[..1], &[upgrade("u", Some("r9"))]);
    assert_eq!(first[0].before, None);
}

#[test]
fn the_text_and_csv_forms_list_every_point_and_upgrade() {
    let runs = vec![run_scores("r10", 0, &trace(), Some(&ledger()))];
    let history = ScoreHistory {
        r#loop: "erdos-779".into(),
        upgrades: marks(&runs, &[upgrade("u-1", Some("r10"))]),
        runs,
    };
    let text = render::text(&history);
    for line in [
        "score history: erdos-779",
        "run r10  started 19700101-000000  baseline 0.1047",
        "  epoch 1  from a6c8 (0.1047)  end 0.1087  seats 45.1 min  scoring 16.5 min",
        "    e1-main-b        main-b     0.1087  2 seeds, 1 failed  promoted",
        "  epoch 2  from 8d2d (0.1087)  end -  seats - min",
        "  u-1  title u-1\n    before -\n    after  run r10 (start 0.1047, best 0.1087, end 0.1087, 1 promoted of 2)",
    ] {
        assert!(text.contains(line.trim_end_matches(" min")), "{line}\n{text}");
    }
    let csv = render::csv(&history);
    let rows: Vec<&str> = csv.lines().collect();
    assert_eq!(rows[0], render::CSV_HEADER);
    assert_eq!(rows[1], "r10,0,0,baseline,a6c8,,0.1047,0,2,false,a6c8,0.1047,,,u-1");
    assert_eq!(
        rows[3],
        "r10,0,1,candidate,e1-main-b,main-b,0.1087,1,2,true,a6c8,0.1047,2706,988,u-1"
    );
    assert_eq!(rows.len(), 4);
    let empty = ScoreHistory {
        r#loop: "l".into(),
        runs: Vec::new(),
        upgrades: Vec::new(),
    };
    assert_eq!(render::text(&empty), "score history: l\nno stored runs\n");
}

#[test]
fn collect_reads_every_stored_run_oldest_first_and_skips_ones_without_a_trace() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::new(home.path());
    for (id, at, events) in [("b", 20, true), ("a", 10, true), ("c", 30, false)] {
        let run = store
            .create_run("l", Some(id), &BTreeMap::new())
            .unwrap();
        write_json(
            &run.record_path(),
            &RunRecord {
                id: id.into(),
                r#loop: "l".into(),
                status: RunStatus::Finished,
                pid: 0,
                started_at: at,
                finished_at: None,
                stop_reason: None,
                incumbent: String::new(),
                incumbent_score: None,
                epochs: 1,
                spend_usd: 0.0,
                error: None,
                warnings: Vec::new(),
            },
        )
        .unwrap();
        if events {
            fs::write(run.events_path(), trace()).unwrap();
            write_json(&run.ledger_path(), &ledger()).unwrap();
        }
    }
    let history = collect(&store, "l", &[upgrade("u", Some("b"))]).unwrap();
    let ids: Vec<&str> = history.runs.iter().map(|r| r.run.as_str()).collect();
    assert_eq!(ids, ["a", "b"]);
    assert_eq!(history.upgrades[0].before.as_ref().unwrap().run, "a");
    let wire = serde_json::to_value(&history).unwrap();
    assert_eq!(wire["loop"], "l");
    assert_eq!(wire["runs"][0]["epochs"][0]["scoring_ms"], 988_538);
    assert_eq!(wire["runs"][0]["epochs"][0]["candidates"][1]["promoted"], true);
    assert!(wire["runs"][0]["epochs"][1].get("scoring_ms").is_none());
}
