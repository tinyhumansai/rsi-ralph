//! The closure-backed scorer.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::Error;

#[test]
fn a_uniform_scorer_clamps_and_repeats_its_quality_per_seed() {
    let scorer = ScriptedScorer::uniform(|n: &i32| f64::from(*n) / 10.0);
    let goal = Goal::new("g");
    assert_eq!(
        scorer.assess(&4, &goal, &[1, 2]).unwrap().qualities,
        vec![0.4, 0.4]
    );
    let clamped = scorer.assess(&40, &goal, &[1]).unwrap();
    assert_eq!(clamped.eligible, Some(vec![1.0]));
    assert_eq!(format!("{scorer:?}"), "ScriptedScorer { .. }");
}

#[test]
fn a_closure_scorer_passes_its_error_through() {
    let scorer: ScriptedScorer<str> = ScriptedScorer::new(|_, _, _| Err(Error::JudgeMalformed));
    assert_eq!(
        scorer.assess("s", &Goal::new("g"), &[1]),
        Err(Error::JudgeMalformed)
    );
}

#[test]
fn a_uniform_scorer_rejects_a_non_finite_quality() {
    let goal = Goal::new("g");
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let scorer = ScriptedScorer::uniform(move |_: &str| bad);
        assert_eq!(
            scorer.assess("s", &goal, &[1, 2]),
            Err(Error::InvalidEvaluation),
            "{bad} is not a quality"
        );
    }
}
