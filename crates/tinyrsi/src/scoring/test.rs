//! Formula, resampling, pairing, and monitoring regressions.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::{Budget, EvalDecision, EvalRun};

fn health() -> ScoreHealth {
    ScoreHealth {
        false_claim_rate: 0.0,
        tamper_rate: 0.0,
        cost_ratio: 1.0,
    }
}
fn policy(families: &[&str]) -> ScoringPolicy {
    ScoringPolicy::new(
        families
            .iter()
            .map(|family| FamilyAnchor {
                family: (*family).into(),
                baseline: 0.2,
                reference: 0.8,
                weight: 1.0,
                regression_limit: 0.0,
            })
            .collect(),
        ScorePenalties {
            false_claims: 2.0,
            tamper: 3.0,
            cost: 0.5,
        },
        0.01,
        0.1,
        200,
        42,
    )
    .unwrap()
}
fn runs(family: &str, quality: f64) -> Vec<EvalRun> {
    ["first", "second"]
        .into_iter()
        .flat_map(|task| {
            (0..3).map(move |seed| EvalRun {
                task: format!("{family}-{task}"),
                family: family.into(),
                seed,
                quality,
                tokens: 10,
                false_claims: 0,
                tamper: 0,
            })
        })
        .collect()
}
fn budget() -> Budget {
    Budget { max_tokens: 20 }
}

#[test]
fn formula_normalizes_family_quality_and_subtracts_health_penalties() {
    let telemetry = ScoreHealth {
        false_claim_rate: 0.1,
        tamper_rate: 0.1,
        cost_ratio: 1.2,
    };
    let estimate = score(
        &runs("code", 0.5),
        &telemetry,
        &budget(),
        &policy(&["code"]),
    )
    .unwrap();
    assert!((estimate.per_family["code"] - 0.5).abs() < 1e-12);
    assert!((estimate.value + 0.1).abs() < 1e-12);
    assert!((estimate.lower - estimate.upper).abs() < 1e-12);
    let efficient = ScoreHealth {
        cost_ratio: 0.5,
        ..health()
    };
    assert!(
        (score(
            &runs("code", 0.5),
            &efficient,
            &budget(),
            &policy(&["code"])
        )
        .unwrap()
        .value
            - 0.5)
            .abs()
            < 1e-12
    );
}

#[test]
fn fractional_iqm_handles_one_two_and_three_families() {
    assert!((iqm(&[0.0]) - 0.0).abs() < 1e-12);
    assert!((iqm(&[0.0, 1.0]) - 0.5).abs() < 1e-12);
    assert!((iqm(&[0.0, 0.0, 1.0]) - 1.0 / 6.0).abs() < 1e-12);
    assert!((iqm(&[0.0, 0.2, 0.8, 1.0]) - 0.5).abs() < 1e-12);
}

#[test]
fn bootstrap_is_reproducible_and_pairs_remove_task_seed_noise() {
    let mut incumbent = runs("code", 0.3);
    for (index, run) in incumbent.iter_mut().enumerate() {
        run.quality += match index % 3 {
            0 => 0.0,
            1 => 0.1,
            _ => 0.2,
        };
    }
    let mut candidate = incumbent.clone();
    for run in &mut candidate {
        run.quality += 0.1;
    }
    let scoring = policy(&["code"]);
    let first = compare_h(
        &incumbent,
        &candidate,
        &health(),
        &health(),
        &budget(),
        &scoring,
    )
    .unwrap();
    let second = compare_h(
        &incumbent,
        &candidate,
        &health(),
        &health(),
        &budget(),
        &scoring,
    )
    .unwrap();
    assert_eq!(first, second);
    assert!((first.delta - 1.0 / 6.0).abs() < 1e-12);
    assert!((first.upper - first.lower).abs() < 1e-12);
    assert!(first.incumbent.upper > first.incumbent.lower);
    assert!(matches!(first.decision, EvalDecision::Accept { .. }));
}

#[test]
fn rejects_normalized_family_health_and_cost_regressions() {
    let mut incumbent = runs("code", 0.4);
    incumbent.extend(runs("research", 0.4));
    let mut candidate = runs("code", 0.8);
    candidate.extend(runs("research", 0.3));
    assert!(matches!(
        compare_h(
            &incumbent,
            &candidate,
            &health(),
            &health(),
            &budget(),
            &policy(&["code", "research"])
        )
        .unwrap()
        .decision,
        EvalDecision::Reject { .. }
    ));
    for field in 0..3 {
        let mut telemetry = health();
        match field {
            0 => telemetry.false_claim_rate = 0.1,
            1 => telemetry.tamper_rate = 0.1,
            _ => telemetry.cost_ratio = 1.2,
        }
        let mut candidate = runs("code", 0.8);
        if field == 2 {
            for run in &mut candidate {
                run.tokens = 12;
            }
        }
        assert!(matches!(
            compare_h(
                &runs("code", 0.4),
                &candidate,
                &health(),
                &telemetry,
                &budget(),
                &policy(&["code"])
            )
            .unwrap()
            .decision,
            EvalDecision::Reject { .. }
        ));
    }
}

#[test]
fn invalid_anchors_rates_caps_pairing_and_resample_count_are_errors() {
    let anchor = FamilyAnchor {
        family: "code".into(),
        baseline: 0.2,
        reference: 0.8,
        weight: 1.0,
        regression_limit: 0.0,
    };
    for field in 0..5 {
        let mut invalid = anchor.clone();
        match field {
            0 => invalid.reference = 0.2,
            1 => invalid.weight = 0.0,
            2 => invalid.baseline = f64::NAN,
            3 => invalid.regression_limit = -1.0,
            _ => invalid.family = " ".into(),
        }
        assert!(
            ScoringPolicy::new(
                vec![invalid],
                ScorePenalties {
                    false_claims: 0.0,
                    tamper: 0.0,
                    cost: 0.0
                },
                0.01,
                0.1,
                200,
                0
            )
            .is_err()
        );
    }
    assert!(
        ScoringPolicy::new(
            vec![anchor],
            ScorePenalties {
                false_claims: 0.0,
                tamper: 0.0,
                cost: 0.0
            },
            0.01,
            0.1,
            199,
            0
        )
        .is_err()
    );
    let mut invalid = health();
    invalid.false_claim_rate = f64::NAN;
    assert!(score(&runs("code", 0.4), &invalid, &budget(), &policy(&["code"])).is_err());
    assert!(
        score(
            &runs("other", 0.4),
            &health(),
            &budget(),
            &policy(&["code"])
        )
        .is_err()
    );
    assert!(
        score(
            &runs("code", 0.4),
            &health(),
            &Budget { max_tokens: 9 },
            &policy(&["code"])
        )
        .is_err()
    );
    assert!(
        compare_h(
            &runs("code", 0.4),
            &runs("other", 0.5),
            &health(),
            &health(),
            &budget(),
            &policy(&["code"])
        )
        .is_err()
    );
    let mismatched = ScoreHealth {
        cost_ratio: 1.2,
        ..health()
    };
    assert!(
        compare_h(
            &runs("code", 0.4),
            &runs("code", 0.5),
            &health(),
            &mismatched,
            &budget(),
            &policy(&["code"])
        )
        .is_err()
    );
}

#[test]
fn anchor_ledger_detects_sustained_loss_and_widening_gap() {
    let scoring = policy(&["code"]);
    let baseline = score(&runs("code", 0.6), &health(), &budget(), &scoring).unwrap();
    let weak = score(&runs("code", 0.3), &health(), &budget(), &scoring).unwrap();
    let mut ledger = AnchorLedger::new(2, 0.01, 0.01).unwrap();
    assert!(!ledger.record(0, &baseline, &baseline).unwrap().regression);
    let first = ledger.record(1, &baseline, &weak).unwrap();
    assert!(!first.regression && !first.widening_gap);
    let second = ledger.record(2, &baseline, &weak).unwrap();
    assert!(second.regression && second.widening_gap);
    assert_eq!(second.best_epoch, 0);
    let recovered = ledger.record(3, &baseline, &baseline).unwrap();
    assert!(!recovered.regression && !recovered.widening_gap);
    assert!(ledger.record(3, &baseline, &baseline).is_err());
}

#[test]
fn bootstrap_resamples_task_clusters_and_applies_family_weights() {
    let mut evidence = runs("code", 0.2);
    for run in &mut evidence {
        if run.task.ends_with("second") {
            run.quality = 0.8;
        }
    }
    let estimate = score(&evidence, &health(), &budget(), &policy(&["code"])).unwrap();
    assert!(estimate.upper - estimate.lower > 0.5);
    let scoring = ScoringPolicy::new(
        vec![FamilyAnchor {
            family: "code".into(),
            baseline: 0.2,
            reference: 0.8,
            weight: 2.0,
            regression_limit: 0.0,
        }],
        ScorePenalties {
            false_claims: 0.0,
            tamper: 0.0,
            cost: 0.0,
        },
        0.01,
        0.1,
        200,
        0,
    )
    .unwrap();
    assert!(
        (score(&runs("code", 0.5), &health(), &budget(), &scoring)
            .unwrap()
            .value
            - 1.0)
            .abs()
            < 1e-12
    );
}

#[test]
fn malformed_policy_telemetry_and_numeric_overflow_are_rejected() {
    let anchors = vec![FamilyAnchor {
        family: "code".into(),
        baseline: 0.2,
        reference: 0.8,
        weight: 1.0,
        regression_limit: 0.0,
    }];
    let penalties = ScorePenalties {
        false_claims: 0.0,
        tamper: 0.0,
        cost: 0.0,
    };
    assert!(ScoringPolicy::new(vec![], penalties.clone(), 0.0, 0.0, 200, 0).is_err());
    assert!(
        ScoringPolicy::new(
            vec![anchors[0].clone(), anchors[0].clone()],
            penalties.clone(),
            0.0,
            0.0,
            200,
            0
        )
        .is_err()
    );
    for field in 0..5 {
        let mut p = penalties.clone();
        let mut margin = 0.0;
        let mut tolerance = 0.0;
        match field {
            0 => p.false_claims = f64::NAN,
            1 => p.tamper = -1.0,
            2 => p.cost = f64::INFINITY,
            3 => margin = -0.1,
            _ => tolerance = f64::NAN,
        }
        assert!(ScoringPolicy::new(anchors.clone(), p, margin, tolerance, 200, 0).is_err());
    }
    for field in 0..3 {
        let mut telemetry = health();
        match field {
            0 => telemetry.false_claim_rate = 1.1,
            1 => telemetry.tamper_rate = -1.0,
            _ => telemetry.cost_ratio = f64::INFINITY,
        }
        assert!(
            score(
                &runs("code", 0.5),
                &telemetry,
                &budget(),
                &policy(&["code"])
            )
            .is_err()
        );
    }
    let tiny_range = ScoringPolicy::new(
        vec![FamilyAnchor {
            baseline: 0.0,
            reference: f64::MIN_POSITIVE / 100.0,
            ..anchors[0].clone()
        }],
        penalties.clone(),
        0.0,
        0.0,
        200,
        0,
    )
    .unwrap();
    assert!(score(&runs("code", 1.0), &health(), &budget(), &tiny_range).is_err());
    let huge_weight = ScoringPolicy::new(
        vec![FamilyAnchor {
            weight: f64::MAX,
            ..anchors[0].clone()
        }],
        penalties,
        0.0,
        0.0,
        200,
        0,
    )
    .unwrap();
    assert!(score(&runs("code", 1.0), &health(), &budget(), &huge_weight).is_err());
}

#[test]
fn zero_token_reference_and_tie_decisions_are_explicit() {
    let mut incumbent = runs("code", 0.5);
    for run in &mut incumbent {
        run.tokens = 0;
    }
    let mut candidate = incumbent.clone();
    let scoring = policy(&["code"]);
    assert!(matches!(
        compare_h(
            &incumbent,
            &candidate,
            &health(),
            &health(),
            &budget(),
            &scoring
        )
        .unwrap()
        .decision,
        EvalDecision::Inconclusive { .. }
    ));
    candidate[0].tokens = 1;
    assert!(
        compare_h(
            &incumbent,
            &candidate,
            &health(),
            &health(),
            &budget(),
            &scoring
        )
        .is_err()
    );
    let cost_not_relative = ScoreHealth {
        cost_ratio: 2.0,
        ..health()
    };
    assert!(
        compare_h(
            &runs("code", 0.5),
            &runs("code", 0.5),
            &cost_not_relative,
            &health(),
            &budget(),
            &scoring
        )
        .is_err()
    );
}

#[test]
fn anchor_monitoring_rejects_bad_evidence_and_resets_skipped_epochs() {
    assert!(AnchorLedger::new(1, 0.0, 0.0).is_err());
    assert!(AnchorLedger::new(2, f64::NAN, 0.0).is_err());
    let baseline = score(&runs("code", 0.6), &health(), &budget(), &policy(&["code"])).unwrap();
    let weak = score(&runs("code", 0.3), &health(), &budget(), &policy(&["code"])).unwrap();
    let mut ledger = AnchorLedger::new(2, 0.01, 0.01).unwrap();
    ledger.record(0, &baseline, &baseline).unwrap();
    ledger.record(1, &baseline, &weak).unwrap();
    assert!(!ledger.record(3, &baseline, &weak).unwrap().regression);
    let mut invalid = baseline.clone();
    invalid.lower = invalid.upper + 1.0;
    assert!(ledger.record(4, &baseline, &invalid).is_err());
    invalid = baseline.clone();
    invalid.value = f64::NAN;
    assert!(ledger.record(4, &baseline, &invalid).is_err());
    let other = score(
        &runs("research", 0.6),
        &health(),
        &budget(),
        &policy(&["research"]),
    )
    .unwrap();
    assert!(ledger.record(4, &other, &other).is_err());
    let strong = score(&runs("code", 0.8), &health(), &budget(), &policy(&["code"])).unwrap();
    assert_eq!(ledger.record(4, &strong, &strong).unwrap().best_epoch, 4);
}

#[test]
fn score_wire_form_and_constant_bootstrap_endpoints_are_stable() {
    let estimate = score(&runs("code", 0.6), &health(), &budget(), &policy(&["code"])).unwrap();
    assert_eq!(estimate.lower.to_bits(), estimate.upper.to_bits());
    let encoded = serde_json::to_value(&estimate).unwrap();
    assert!(encoded.get("per_family").unwrap().get("code").is_some());
    assert_eq!(
        serde_json::from_value::<ScoreEstimate>(encoded).unwrap(),
        estimate
    );
}

#[test]
fn zero_cost_tolerance_preserves_integer_precision() {
    let mut incumbent = runs("code", 0.4);
    for run in &mut incumbent {
        run.tokens = 1_u64 << 54;
    }
    let mut candidate = incumbent.clone();
    for run in &mut candidate {
        run.quality = 0.8;
    }
    candidate[0].tokens += 1;
    let mut scoring = policy(&["code"]);
    scoring.cost_tolerance = 0.0;
    assert!(matches!(
        compare_h(
            &incumbent,
            &candidate,
            &health(),
            &health(),
            &Budget {
                max_tokens: u64::MAX
            },
            &scoring
        )
        .unwrap()
        .decision,
        EvalDecision::Reject { .. }
    ));
}

#[test]
fn scoring_policy_spec_revalidates_serialized_configuration() {
    let original = policy(&["code"]);
    let spec = original.spec();
    let decoded: ScoringPolicySpec =
        serde_json::from_slice(&serde_json::to_vec(&spec).unwrap()).unwrap();
    assert_eq!(decoded.build().unwrap(), original);
    let mut invalid = decoded.clone();
    invalid.resamples = 199;
    assert!(invalid.build().is_err());
    invalid = decoded;
    invalid.anchors.push(invalid.anchors[0].clone());
    assert!(invalid.build().is_err());
}
