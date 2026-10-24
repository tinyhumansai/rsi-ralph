//! The metric-to-quality map and its validation.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;

#[test]
fn a_better_metric_is_always_a_higher_quality_inside_the_unit_interval() {
    let min = MetricScorer::new(MetricDirection::Minimize);
    let max = MetricScorer::new(MetricDirection::Maximize);
    for (a, b) in [(-5.0, -1.0), (0.0, 0.5), (3.0, 1e9)] {
        assert!(min.quality(a) > min.quality(b));
        assert!(max.quality(b) > max.quality(a));
        for q in [min.quality(a), max.quality(b)] {
            assert!(q > 0.0 && q < 1.0);
        }
    }
    assert!((max.quality(0.0) - 0.5).abs() < 1e-12);
}

#[test]
fn reaches_and_better_follow_the_direction() {
    let min = MetricScorer::new(MetricDirection::Minimize);
    assert!(min.reaches(0.05, 0.06) && min.reaches(0.06, 0.06) && !min.reaches(0.07, 0.06));
    assert!(min.better(0.1, 0.2) && !min.better(0.2, 0.2));
    let max = MetricScorer::new(MetricDirection::Maximize);
    assert!(max.reaches(0.9, 0.8) && max.better(0.9, 0.8));
}

#[test]
fn assesses_eligible_qualities_and_keeps_the_raw_metric() {
    let scorer = MetricScorer::new(MetricDirection::Maximize);
    let a = scorer
        .assess(&[1.0, 2.0], &Goal::new("g"), &[7, 8])
        .unwrap();
    assert_eq!(a.eligible.as_ref(), Some(&a.qualities));
    assert_eq!(a.metric.unwrap().values, vec![1.0, 2.0]);
    assert_eq!(a.tokens, 0);
}

#[test]
fn refuses_a_missing_or_non_finite_value() {
    let scorer = MetricScorer::new(MetricDirection::Maximize);
    let goal = Goal::new("g");
    assert_eq!(
        scorer.assess(&[1.0], &goal, &[1, 2]),
        Err(Error::InvalidEvaluation)
    );
    assert_eq!(
        scorer.assess(&[1.0, f64::NAN], &goal, &[1, 2]),
        Err(Error::InvalidEvaluation)
    );
}
