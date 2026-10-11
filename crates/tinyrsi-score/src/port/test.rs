//! The decision rule: `compare`'s verdict, the eligibility gate and units.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::Error;

const SEEDS: [u64; 3] = [1, 2, 3];

pub(crate) fn policy() -> EvalPolicy {
    EvalPolicy {
        margin: 0.0,
        standard_error_multiplier: 2.0,
        family_regression_limit: 1.0,
        cost_tolerance: 0.0,
    }
}

/// Qualities around `q` with a little spread, eligible unless told otherwise.
pub(crate) fn at(q: f64, eligible: bool) -> Assessment {
    let qualities = vec![q, q + 0.01, q - 0.01];
    Assessment {
        eligible: eligible.then(|| qualities.clone()),
        qualities,
        metric: None,
        rationale: None,
        tokens: 0,
    }
}

#[test]
fn accepts_a_clear_eligible_improvement() {
    let d = decide(&at(0.2, true), &at(0.8, true), &SEEDS, &policy()).unwrap();
    assert_eq!(d.decision, ScoreDecision::Accept);
    // Every seed gains exactly 0.6, so the interval has zero width.
    assert!((d.estimate - 0.6).abs() < 1e-9);
    assert!((d.lower - d.estimate).abs() < 1e-9 && (d.upper - d.estimate).abs() < 1e-9);
}

#[test]
fn rejects_a_clear_regression_and_reports_an_overlap_as_inconclusive() {
    let worse = decide(&at(0.8, true), &at(0.2, true), &SEEDS, &policy()).unwrap();
    assert_eq!(worse.decision, ScoreDecision::Reject);
    let noisy = Assessment {
        qualities: vec![0.6, 0.4, 0.55],
        ..at(0.5, true)
    };
    let flat = Assessment {
        qualities: vec![0.5, 0.5, 0.5],
        ..at(0.5, true)
    };
    let overlap = decide(&flat, &noisy, &SEEDS, &policy()).unwrap();
    assert_eq!(overlap.decision, ScoreDecision::Inconclusive);
}

#[test]
fn caps_an_accept_without_eligible_evidence_at_inconclusive() {
    for (incumbent, candidate) in [(true, false), (false, true), (false, false)] {
        let d = decide(&at(0.2, incumbent), &at(0.8, candidate), &SEEDS, &policy()).unwrap();
        assert_eq!(d.decision, ScoreDecision::Inconclusive);
    }
}

#[test]
fn caps_an_accept_the_eligible_evidence_does_not_support() {
    let mut incumbent = at(0.2, true);
    let mut candidate = at(0.8, true);
    incumbent.eligible = Some(vec![0.5, 0.51, 0.49]);
    candidate.eligible = Some(vec![0.5, 0.51, 0.49]);
    let d = decide(&incumbent, &candidate, &SEEDS, &policy()).unwrap();
    assert_eq!(d.decision, ScoreDecision::Inconclusive);
}

#[test]
fn reports_the_interval_in_metric_units_when_both_sides_measured_one() {
    let with = |values: Vec<f64>, a: Assessment| Assessment {
        metric: Some(MetricEvidence {
            values,
            direction: MetricDirection::Minimize,
        }),
        ..a
    };
    let incumbent = with(vec![1.0, 1.2, 0.8], at(0.2, true));
    let candidate = with(vec![0.5, 0.6, 0.4], at(0.8, true));
    let d = decide(&incumbent, &candidate, &SEEDS, &policy()).unwrap();
    // Minimizing: improvements are 0.5, 0.6, 0.4 → mean 0.5, sd 0.1.
    assert!((d.estimate - 0.5).abs() < 1e-9);
    let half = 2.0 * (0.01_f64 / 3.0).sqrt();
    assert!((d.lower - (0.5 - half)).abs() < 1e-9);
    assert!((d.upper - (0.5 + half)).abs() < 1e-9);
    // Only a side without a metric falls back to quality units.
    let q = decide(&at(0.2, true), &candidate, &SEEDS, &policy()).unwrap();
    assert!((q.estimate - 0.6).abs() < 1e-9);
}

#[test]
fn refuses_contradictory_or_misaligned_metric_evidence() {
    let with = |values: Vec<f64>, direction| Assessment {
        metric: Some(MetricEvidence { values, direction }),
        ..at(0.5, true)
    };
    let incumbent = with(vec![1.0, 1.2, 0.8], MetricDirection::Minimize);
    let flipped = with(vec![0.5, 0.6, 0.4], MetricDirection::Maximize);
    assert_eq!(
        decide(&incumbent, &flipped, &SEEDS, &policy()),
        Err(crate::Error::InvalidEvaluation)
    );
    let short = with(vec![0.5], MetricDirection::Minimize);
    assert_eq!(
        decide(&incumbent, &short, &SEEDS, &policy()),
        Err(crate::Error::InvalidEvaluation)
    );
    // A lone metric that does not match the seeds is malformed too.
    assert_eq!(
        decide(&at(0.5, true), &short, &SEEDS, &policy()),
        Err(crate::Error::InvalidEvaluation)
    );
}

#[test]
fn refuses_misaligned_or_out_of_range_qualities() {
    let short = Assessment {
        qualities: vec![0.5, 0.5],
        ..at(0.5, true)
    };
    assert_eq!(
        decide(&short, &at(0.5, true), &SEEDS, &policy()),
        Err(Error::InvalidEvaluation)
    );
    let out = Assessment {
        qualities: vec![1.5, 0.5, 0.5],
        ..at(0.5, true)
    };
    assert_eq!(
        decide(&at(0.5, true), &out, &SEEDS, &policy()),
        Err(Error::InvalidEvaluation)
    );
    assert_eq!(
        decide(&at(0.2, true), &at(0.8, true), &[1, 1, 1], &policy()),
        Err(Error::InvalidEvaluation)
    );
}

#[test]
fn a_boxed_scorer_is_a_scorer() {
    let boxed: Box<dyn Scorer<str>> = Box::new(crate::ScriptedScorer::uniform(|s: &str| {
        if s == "good" { 0.9 } else { 0.1 }
    }));
    let a = boxed.assess("good", &Goal::new("g"), &SEEDS).unwrap();
    assert_eq!(a.qualities, vec![0.9; 3]);
}

#[test]
fn refuses_a_metric_interval_that_overflows() {
    let with = |values: Vec<f64>, a: Assessment| Assessment {
        metric: Some(MetricEvidence {
            values,
            direction: MetricDirection::Maximize,
        }),
        ..a
    };
    let incumbent = with(vec![-f64::MAX, -f64::MAX, -f64::MAX], at(0.2, true));
    let candidate = with(vec![f64::MAX, f64::MAX, f64::MAX], at(0.8, true));
    assert_eq!(
        decide(&incumbent, &candidate, &SEEDS, &policy()),
        Err(Error::InvalidEvaluation)
    );
}

#[test]
fn validates_eligible_evidence_whatever_the_primary_verdict() {
    for eligible in [vec![0.5], vec![0.5, f64::NAN, 0.5], vec![0.5, 2.0, 0.5]] {
        let mut malformed = at(0.8, true);
        malformed.eligible = Some(eligible);
        // A regression: the primary comparison rejects, but the malformed
        // eligible evidence must still be refused.
        assert_eq!(
            decide(&malformed, &at(0.2, true), &SEEDS, &policy()),
            Err(Error::InvalidEvaluation)
        );
    }
}

/// Eligible metric evidence with neutral qualities.
fn measured(values: &[f64], direction: MetricDirection) -> Assessment {
    Assessment {
        qualities: vec![0.5; values.len()],
        eligible: Some(vec![0.5; values.len()]),
        metric: Some(MetricEvidence {
            values: values.to_vec(),
            direction,
        }),
        rationale: None,
        tokens: 0,
    }
}

#[test]
fn dominance_is_no_loss_on_any_seed_and_a_gain_on_one() {
    let min = MetricDirection::Minimize;
    let max = MetricDirection::Maximize;
    let incumbent = measured(&[14.0, 12.0, 13.0], min);
    assert!(dominates(&incumbent, &measured(&[6.0, 6.0, 13.0], min)));
    assert!(!dominates(&incumbent, &measured(&[6.0, 6.0, 14.0], min)));
    assert!(!dominates(&incumbent, &incumbent));
    assert!(dominates(
        &measured(&[1.0, 2.0], max),
        &measured(&[1.0, 3.0], max)
    ));
    assert!(!dominates(
        &measured(&[1.0, 2.0], max),
        &measured(&[1.0, 1.0], max)
    ));
}

#[test]
fn dominance_needs_aligned_metric_and_eligible_evidence_on_both_sides() {
    let min = MetricDirection::Minimize;
    let incumbent = measured(&[11.0, 6.0, 6.0], min);
    let better = measured(&[6.0, 6.0, 6.0], min);
    let mut flipped = better.clone();
    flipped.metric.as_mut().unwrap().direction = MetricDirection::Maximize;
    assert!(!dominates(&incumbent, &flipped));
    assert!(!dominates(&incumbent, &measured(&[6.0, 6.0], min)));
    let mut unmeasured = better.clone();
    unmeasured.metric = None;
    assert!(!dominates(&incumbent, &unmeasured));
    let mut ineligible = better.clone();
    ineligible.eligible = None;
    assert!(!dominates(&incumbent, &ineligible));
    let mut short = better.clone();
    short.eligible = Some(vec![0.5; 2]);
    assert!(!dominates(&incumbent, &short));
    // An eligible quality that drops on any seed vetoes the metric's verdict.
    let mut vetoed = better;
    vetoed.eligible = Some(vec![0.5, 0.4, 0.5]);
    assert!(!dominates(&incumbent, &vetoed));
}
