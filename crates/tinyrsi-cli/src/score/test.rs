//! Scoring checkouts, judges, and both decision rules.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::{
    manifest::{DecideSpec, ScorerSpec},
    step::{ScoreResult, Split},
};
use tinyrsi_score::ScriptedScorer;

fn scorer(id: &str, run: Option<&str>, judge: Option<&str>, weight: f64) -> ScorerSpec {
    ScorerSpec {
        id: id.into(),
        run: run.map(str::to_owned),
        judge: judge.map(str::to_owned),
        weight,
        timeout_secs: 20,
        raters: 1,
    }
}

fn seeds_spec(direction: MetricDirection, scorers: Vec<ScorerSpec>) -> ScoreSpec {
    ScoreSpec {
        scorers,
        decide: DecideSpec {
            on: "m".into(),
            test: DecideTest::Seeds,
            seeds: vec![1, 2, 3],
            split: None,
            alpha: 0.1,
            direction,
            margin: 0.0,
            target: Some(6.0),
        },
    }
}

fn card(label: &str, scores: &[f64]) -> Scorecard {
    Scorecard {
        label: label.into(),
        runs: BTreeMap::from([(
            "m".to_owned(),
            scores
                .iter()
                .enumerate()
                .map(|(i, s)| SeedScore {
                    seed: i as u64 + 1,
                    ok: true,
                    reason: None,
                    result: Some(ScoreResult {
                        score: *s,
                        ..ScoreResult::default()
                    }),
                    duration_ms: 1,
                })
                .collect(),
        )]),
        judges: BTreeMap::new(),
        headline: Some(
            scores.iter().sum::<f64>() / f64::from(u32::try_from(scores.len()).unwrap()),
        ),
    }
}

fn ctx(root: &Path) -> StepContext {
    StepContext {
        loop_dir: root.to_path_buf(),
        run_dir: root.to_path_buf(),
        incumbent: root.to_path_buf(),
        epoch: 0,
        model_url: None,
        env: BTreeMap::new(),
        inherited: vec![("PATH".into(), "/usr/bin:/bin".into())],
    }
}

#[test]
fn runs_each_scorer_per_seed_and_records_failures() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("m.sh"), "echo \"METRIC m=$RSI_SEED\"\n").unwrap();
    std::fs::write(dir.path().join("bad.sh"), "echo nope >&2; exit 1\n").unwrap();
    let spec = seeds_spec(
        MetricDirection::Minimize,
        vec![
            scorer("m", Some("/bin/sh m.sh"), None, 1.0),
            scorer("bad", Some("/bin/sh bad.sh"), None, 1.0),
            scorer("q", None, Some("rubric.md"), 0.5),
        ],
    );
    let checkout = Checkout {
        label: "incumbent".into(),
        path: dir.path().to_path_buf(),
    };
    let card = score_checkout(
        &ctx(dir.path()),
        &spec,
        &checkout,
        &dir.path().join("s"),
        &Judges::new(),
        &BTreeMap::new(),
        None,
    )
    .unwrap();
    assert_eq!(card.runs["m"].len(), 3);
    assert_eq!(card.headline, Some(2.0));
    assert!(
        card.runs["bad"]
            .iter()
            .all(|r| !r.ok && r.reason.as_deref() == Some("nope"))
    );
    assert!(card.judges.is_empty());
}

#[test]
fn judges_grade_a_non_empty_change() {
    let dir = tempfile::tempdir().unwrap();
    let spec = seeds_spec(
        MetricDirection::Minimize,
        vec![
            scorer("q", None, Some("r.md"), 1.0),
            scorer("m", Some("true"), None, 1.0),
        ],
    );
    let mut judges = Judges::new();
    judges.insert(
        "q".into(),
        Box::new(ScriptedScorer::uniform(|change: &str| {
            if change.contains("good") { 0.9 } else { 0.1 }
        })),
    );
    let rubrics = BTreeMap::from([("q".to_owned(), "Is the change good?".to_owned())]);
    let checkout = Checkout {
        label: "c".into(),
        path: dir.path().to_path_buf(),
    };
    let card = score_checkout(
        &ctx(dir.path()),
        &spec,
        &checkout,
        &dir.path().join("s"),
        &judges,
        &rubrics,
        Some("+good"),
    )
    .unwrap();
    assert!((card.judges["q"].quality - 0.9).abs() < 1e-12);
    assert_eq!(card.headline, None, "true prints no metric");
    let none = score_checkout(
        &ctx(dir.path()),
        &spec,
        &checkout,
        &dir.path().join("t"),
        &judges,
        &rubrics,
        Some("  "),
    )
    .unwrap();
    assert!(none.judges.is_empty());
}

#[test]
fn seeds_rule_accepts_a_consistent_improvement_and_rejects_a_failed_seed() {
    let spec = seeds_spec(
        MetricDirection::Minimize,
        vec![scorer("m", Some("x"), None, 1.0)],
    );
    let v = decide(
        &spec,
        &card("incumbent", &[9.0, 9.0, 9.0]),
        &card("c", &[7.0, 7.0, 7.1]),
    )
    .unwrap();
    assert_eq!(v.decision, ScoreDecision::Accept);
    assert!(v.estimate > 1.9);
    let v = decide(
        &spec,
        &card("incumbent", &[7.0, 7.0, 7.0]),
        &card("c", &[9.0, 9.0, 9.0]),
    )
    .unwrap();
    assert_eq!(v.decision, ScoreDecision::Reject);

    let mut failed = card("c", &[1.0, 1.0, 1.0]);
    failed.runs.get_mut("m").unwrap()[1] = SeedScore {
        seed: 2,
        ok: false,
        reason: Some("timed out".into()),
        result: None,
        duration_ms: 0,
    };
    let v = decide(&spec, &card("incumbent", &[9.0, 9.0, 9.0]), &failed).unwrap();
    assert_eq!(v.decision, ScoreDecision::Reject);
    assert_eq!(v.reason, "seed 2 failed: timed out");
}

#[test]
fn a_judge_cannot_accept_alone_but_can_temper() {
    let spec = seeds_spec(
        MetricDirection::Minimize,
        vec![
            scorer("m", Some("x"), None, 1.0),
            scorer("q", None, Some("r"), 3.0),
        ],
    );
    let mut flat = card("c", &[9.0, 9.0, 9.0]);
    flat.judges.insert(
        "q".into(),
        JudgeScore {
            quality: 1.0,
            rationale: None,
        },
    );
    let v = decide(&spec, &card("incumbent", &[9.0, 9.0, 9.0]), &flat).unwrap();
    assert_ne!(v.decision, ScoreDecision::Accept);
}

#[test]
fn an_incumbent_without_scores_cannot_be_compared() {
    let spec = seeds_spec(
        MetricDirection::Minimize,
        vec![scorer("m", Some("x"), None, 1.0)],
    );
    let mut empty = card("incumbent", &[1.0]);
    empty.runs.clear();
    assert!(matches!(
        decide(&spec, &empty, &card("c", &[1.0, 1.0, 1.0])),
        Err(Error::Score(_))
    ));
}

fn sign_card(label: &str, items: &[(String, f64)]) -> Scorecard {
    let result = ScoreResult {
        score: 0.0,
        splits: BTreeMap::from([(
            "test".to_owned(),
            Split {
                items: items.iter().cloned().collect(),
            },
        )]),
        ..ScoreResult::default()
    };
    Scorecard {
        label: label.into(),
        runs: BTreeMap::from([(
            "m".to_owned(),
            vec![SeedScore {
                seed: 0,
                ok: true,
                reason: None,
                result: Some(result),
                duration_ms: 0,
            }],
        )]),
        judges: BTreeMap::new(),
        headline: Some(0.0),
    }
}

#[test]
fn sign_rule_uses_the_split_items() {
    let mut spec = seeds_spec(
        MetricDirection::Maximize,
        vec![scorer("m", Some("x"), None, 1.0)],
    );
    spec.decide.test = DecideTest::Sign;
    spec.decide.split = Some("test".into());
    assert_eq!(seeds(&spec), vec![0]);
    let inc: Vec<_> = (0..20).map(|i| (format!("q{i}"), 0.0)).collect();
    let cand: Vec<_> = (0..20)
        .map(|i| (format!("q{i}"), if i < 8 { 1.0 } else { 0.0 }))
        .collect();
    let v = decide(&spec, &sign_card("incumbent", &inc), &sign_card("c", &cand)).unwrap();
    assert_eq!(v.decision, ScoreDecision::Accept);
    assert_eq!(v.sign.unwrap().wins, 8);
    assert!(v.reason.starts_with("sign test on test: 8 wins"));
}

#[test]
fn targets_and_ordering_follow_the_direction() {
    let spec = seeds_spec(MetricDirection::Minimize, vec![]);
    assert!(reaches(&spec, 6.0) && !reaches(&spec, 6.5));
    assert!(better(&spec, 6.0, 7.0));
    let mut sign = spec.clone();
    sign.decide.test = DecideTest::Sign;
    assert!(better(&sign, 0.7, 0.6));
}

#[test]
fn pins_the_scorecard_stored_form() {
    let json = serde_json::to_value(card("c", &[1.0])).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "label": "c",
            "runs": {"m": [{"seed": 1, "ok": true, "reason": null, "duration_ms": 1,
                "result": {"score": 1.0, "splits": {}, "failures": [], "metrics": {}}}]},
            "judges": {}, "headline": 1.0
        })
    );
}
