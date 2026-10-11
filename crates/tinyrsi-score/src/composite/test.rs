//! Weighted combination, the eligibility of a mix, and projections.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::port::test::{at, policy};
use crate::{MetricDirection, MetricScorer, ScoreDecision, ScriptedScorer, decide};

const SEEDS: [u64; 3] = [1, 2, 3];

/// A scorer that returns a fixed assessment whatever the subject.
fn fixed(a: Assessment) -> ScriptedScorer<str> {
    ScriptedScorer::new(move |_, _, _| Ok(a.clone()))
}

fn goal() -> Goal {
    Goal::new("g")
}

#[test]
fn takes_the_weighted_mean_and_renormalizes_eligible_members() {
    let mut judge = at(0.9, false);
    judge.rationale = Some("judge says yes".into());
    judge.tokens = 40;
    let mix = Composite::new()
        .with(0.75, fixed(judge))
        .with(0.25, fixed(at(0.1, true)));
    let a = mix.assess("s", &goal(), &SEEDS).unwrap();
    assert!((a.qualities[0] - (0.75 * 0.9 + 0.25 * 0.1)).abs() < 1e-12);
    assert_eq!(a.eligible.unwrap(), at(0.1, true).qualities);
    assert_eq!(a.rationale.as_deref(), Some("judge says yes"));
    assert_eq!(a.tokens, 40);
    assert!(a.metric.is_none());
    assert!(format!("{mix:?}").contains("0.75"));
}

#[test]
fn has_no_eligible_evidence_when_no_member_does() {
    let mix = Composite::new()
        .with(0.5, fixed(at(0.9, false)))
        .with(0.5, fixed(at(0.8, false)));
    assert!(mix.assess("s", &goal(), &SEEDS).unwrap().eligible.is_none());
}

#[test]
fn a_heavy_uncalibrated_judge_cannot_accept_over_a_flat_eligible_scorer() {
    let incumbent = Composite::new()
        .with(0.9, fixed(at(0.1, false)))
        .with(0.1, fixed(at(0.5, true)));
    let candidate = Composite::new()
        .with(0.9, fixed(at(0.95, false)))
        .with(0.1, fixed(at(0.5, true)));
    let i = incumbent.assess("old", &goal(), &SEEDS).unwrap();
    let c = candidate.assess("new", &goal(), &SEEDS).unwrap();
    let d = decide(&i, &c, &SEEDS, &policy()).unwrap();
    assert_eq!(d.decision, ScoreDecision::Inconclusive);
}

#[test]
fn keeps_the_metric_only_for_a_single_measuring_member() {
    const HIGH: [f64; 3] = [1.0; 3];
    let metric = || {
        ScriptedScorer::new(move |_: &str, goal: &Goal, seeds: &[u64]| {
            MetricScorer::new(MetricDirection::Maximize).assess(&HIGH, goal, seeds)
        })
    };
    let alone = Composite::new().with(1.0, metric());
    let kept = alone.assess("s", &goal(), &SEEDS).unwrap().metric.unwrap();
    assert_eq!(kept.values, vec![1.0; 3]);
    // A blend decides on blended qualities, so it reports quality units.
    let blend = Composite::new()
        .with(0.5, metric())
        .with(0.5, fixed(at(0.5, true)));
    assert!(blend.assess("s", &goal(), &SEEDS).unwrap().metric.is_none());
}

#[test]
fn refuses_bad_weights_and_misaligned_members() {
    let empty: Composite<'_, str> = Composite::default();
    assert_eq!(
        empty.assess("s", &goal(), &SEEDS),
        Err(Error::InvalidEvaluation)
    );
    for weights in [[0.5, 0.4], [1.2, -0.2], [f64::NAN, 1.0]] {
        let mix = Composite::new()
            .with(weights[0], fixed(at(0.5, true)))
            .with(weights[1], fixed(at(0.5, true)));
        assert_eq!(
            mix.assess("s", &goal(), &SEEDS),
            Err(Error::InvalidEvaluation)
        );
    }
    let short = Assessment {
        qualities: vec![0.5],
        ..at(0.5, true)
    };
    let mix = Composite::new().with(1.0, fixed(short));
    assert_eq!(
        mix.assess("s", &goal(), &SEEDS),
        Err(Error::InvalidEvaluation)
    );
    let mut misaligned = at(0.5, true);
    misaligned.eligible = Some(vec![0.5]);
    let mix = Composite::new().with(1.0, fixed(misaligned));
    assert_eq!(
        mix.assess("s", &goal(), &SEEDS),
        Err(Error::InvalidEvaluation)
    );
}

#[test]
fn passes_a_member_error_through() {
    let failing: ScriptedScorer<str> = ScriptedScorer::new(|_, _, _| Err(Error::HostFailed));
    let mix = Composite::new().with(1.0, failing);
    assert_eq!(mix.assess("s", &goal(), &SEEDS), Err(Error::HostFailed));
}

#[test]
fn a_projection_scores_one_view_of_a_richer_subject() {
    struct Candidate {
        metrics: Vec<f64>,
    }
    let scorer = Projected::new(
        |c: &Candidate| c.metrics.as_slice(),
        MetricScorer::new(MetricDirection::Minimize),
    );
    let a = scorer
        .assess(
            &Candidate {
                metrics: vec![0.1, 0.2, 0.3],
            },
            &goal(),
            &SEEDS,
        )
        .unwrap();
    assert_eq!(a.metric.unwrap().values, vec![0.1, 0.2, 0.3]);
    assert!(format!("{scorer:?}").contains("Minimize"));
}

#[test]
fn rejects_member_qualities_outside_the_unit_interval_before_blending() {
    // 2.0 and 0.0 would blend to a valid-looking 1.0.
    let mut wild = at(0.5, true);
    wild.qualities = vec![2.0; 3];
    let mix = Composite::new()
        .with(0.5, fixed(wild))
        .with(0.5, fixed(at(0.1, false)));
    assert_eq!(
        mix.assess("s", &goal(), &SEEDS),
        Err(Error::InvalidEvaluation)
    );
    let mut wild_eligible = at(0.5, true);
    wild_eligible.eligible = Some(vec![f64::NAN; 3]);
    let mix = Composite::new()
        .with(0.5, fixed(wild_eligible))
        .with(0.5, fixed(at(0.5, true)));
    assert_eq!(
        mix.assess("s", &goal(), &SEEDS),
        Err(Error::InvalidEvaluation)
    );
}

#[test]
fn saturates_member_token_totals_instead_of_overflowing() {
    let mut heavy = at(0.5, true);
    heavy.tokens = u64::MAX;
    let mix = Composite::new()
        .with(0.5, fixed(heavy.clone()))
        .with(0.5, fixed(heavy));
    assert_eq!(mix.assess("s", &goal(), &SEEDS).unwrap().tokens, u64::MAX);
}
