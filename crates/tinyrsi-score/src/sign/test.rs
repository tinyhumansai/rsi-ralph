//! Sign-test p-values, decisions and the stored form.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;

fn scores(values: &[(&str, f64)]) -> BTreeMap<String, f64> {
    values.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

fn range(n: usize, f: impl Fn(usize) -> f64) -> BTreeMap<String, f64> {
    (0..n).map(|i| (format!("q{i}"), f(i))).collect()
}

#[test]
fn p_values_match_hand_computed_binomial_tails() {
    assert!((sign_test_p(3, 0) - 0.125).abs() < 1e-12);
    assert!((sign_test_p(0, 0) - 1.0).abs() < 1e-12);
    assert!((sign_test_p(5, 1) - 7.0 / 64.0).abs() < 1e-12);
    assert!((sign_test_p(0, 4) - 1.0).abs() < 1e-12);
}

#[test]
fn p_values_stay_finite_for_thousands_of_items() {
    let p = sign_test_p(1100, 900);
    assert!(p.is_finite() && p > 0.0 && p < 1e-5);
}

#[test]
fn accepts_a_clear_paired_improvement() {
    let result = sign_test(
        &range(20, |_| 0.0),
        &range(20, |i| if i < 8 { 1.0 } else { 0.0 }),
        0.10,
    )
    .unwrap();
    assert_eq!(result.decision, ScoreDecision::Accept);
    assert_eq!((result.wins, result.losses, result.items), (8, 0, 20));
    assert!(result.lower > 0.0);
}

#[test]
fn noise_is_inconclusive() {
    let inc = scores(&[("a", 1.0), ("b", 0.0), ("c", 1.0), ("d", 0.0)]);
    let cand = scores(&[("a", 0.0), ("b", 1.0), ("c", 1.0), ("d", 0.0)]);
    assert_eq!(
        sign_test(&inc, &cand, 0.10).unwrap().decision,
        ScoreDecision::Inconclusive
    );
}

#[test]
fn rejects_a_clear_regression() {
    let result = sign_test(
        &range(10, |_| 1.0),
        &range(10, |i| if i < 6 { 0.0 } else { 1.0 }),
        0.10,
    )
    .unwrap();
    assert_eq!(result.decision, ScoreDecision::Reject);
}

#[test]
fn compares_only_shared_items() {
    let result = sign_test(&scores(&[("a", 1.0)]), &scores(&[("b", 1.0)]), 0.10).unwrap();
    assert_eq!(result.decision, ScoreDecision::Inconclusive);
    assert_eq!(result.items, 0);
}

#[test]
fn rejects_an_alpha_outside_the_unit_interval_and_non_finite_scores() {
    let a = scores(&[("a", 1.0)]);
    for alpha in [0.0, 1.0, f64::NAN] {
        assert_eq!(sign_test(&a, &a, alpha), Err(Error::InvalidEvaluation));
    }
    let bad = scores(&[("a", f64::NAN)]);
    assert_eq!(sign_test(&a, &bad, 0.1), Err(Error::InvalidEvaluation));
}

#[test]
fn pins_the_stored_form() {
    let result = sign_test(&scores(&[("a", 0.0)]), &scores(&[("a", 1.0)]), 0.6).unwrap();
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "decision": "accept", "items": 1, "wins": 1, "losses": 0,
            "p": 0.5, "estimate": 1.0, "lower": 1.0, "upper": 1.0
        })
    );
    assert_eq!(serde_json::from_value::<SignTest>(json).unwrap(), result);
}
