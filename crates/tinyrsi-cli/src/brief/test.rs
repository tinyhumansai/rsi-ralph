//! Placeholder filling, appended sections and failure sampling.
use super::*;
use crate::{score::SeedScore, step::ScoreResult};

fn card(failures: Vec<Failure>) -> Scorecard {
    let result = ScoreResult {
        score: 0.5,
        failures,
        metrics: BTreeMap::from([("tokens".to_owned(), 812.0)]),
        private: BTreeMap::from([("test_accuracy".to_owned(), 0.61)]),
        ..ScoreResult::default()
    };
    let other = ScoreResult {
        score: 3.0,
        ..ScoreResult::default()
    };
    Scorecard {
        label: "incumbent".into(),
        runs: BTreeMap::from([
            (
                "acc".to_owned(),
                vec![SeedScore {
                    seed: 0,
                    ok: true,
                    reason: None,
                    result: Some(result),
                    duration_ms: 0,
                }],
            ),
            (
                "speed".to_owned(),
                vec![SeedScore {
                    seed: 0,
                    ok: true,
                    reason: None,
                    result: Some(other),
                    duration_ms: 0,
                }],
            ),
        ]),
        judges: BTreeMap::new(),
        headline: Some(0.5),
    }
}

fn failure(id: &str, kind: &str) -> Failure {
    Failure {
        id: id.into(),
        kind: Some(kind.into()),
        text: format!("{id} went wrong"),
    }
}

#[test]
fn fills_placeholders() {
    let card = card(vec![failure("q1", "temporal")]);
    let history = [HistoryEntry {
        epoch: 1,
        candidate: "e1-a".into(),
        outcome: "rejected".into(),
        reason: "worse".into(),
    }];
    let ideas = ["rank by recency".to_owned()];
    let out = render(
        "# Goal\n{{incumbent}}\n{{history}}\n{{failures}}\n{{ideas}}",
        &BriefInput {
            incumbent: &card,
            decide_on: "acc",
            history: &history,
            ideas: &ideas,
        },
    );
    assert!(out.contains("- `acc` (deciding): 0.5000"));
    assert!(out.contains("- `speed`: 3.0000"));
    assert!(out.contains("- tokens: 812"));
    assert!(!out.contains("test_accuracy") && !out.contains("0.61"));
    assert!(out.contains("- epoch 1 `e1-a`: rejected (worse)"));
    assert!(out.contains("### q1 (temporal)\n\nq1 went wrong"));
    assert!(out.contains("- rank by recency"));
    assert!(!out.contains("## Where") && !out.contains("## Ideas"));
}

#[test]
fn appends_missing_sections_and_marks_empty_ones() {
    let card = card(vec![failure("q1", "")]);
    let out = render(
        "Do better.\n\n{{history}}",
        &BriefInput {
            incumbent: &card,
            decide_on: "acc",
            history: &[],
            ideas: &[],
        },
    );
    assert!(out.starts_with("Do better.\n\nNone yet."));
    assert!(out.contains("## Where we are"));
    assert!(out.contains("## What still fails\n\n### q1\n"));
    assert!(!out.contains("## Ideas"));
}

#[test]
fn samples_at_most_two_failures_per_type_and_ten_overall() {
    let mut many: Vec<Failure> = (0..5).map(|i| failure(&format!("a{i}"), "a")).collect();
    many.extend((0..30).map(|i| failure(&format!("x{i}"), &format!("t{i}"))));
    let card = card(many);
    let out = failures(&BriefInput {
        incumbent: &card,
        decide_on: "acc",
        history: &[],
        ideas: &[],
    });
    assert_eq!(out.matches("### ").count(), 10);
    assert_eq!(out.matches("(a)").count(), 2);
}

#[test]
fn clipping_marks_the_cut_and_closes_an_open_fence() {
    assert_eq!(clip("short", 10), "short");
    assert_eq!(clip("abcdef", 3), "abc…");
    let fenced = format!("Q\n```\n{}\n```", "x".repeat(50));
    let cut = clip(&fenced, 20);
    assert!(cut.ends_with("…\n```"), "{cut}");
    assert_eq!(cut.matches("```").count(), 2);
}
