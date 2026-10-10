//! Behavioral tests for paired evaluation and independent hold-outs.

#![allow(clippy::unwrap_used)]

use super::*;

fn runs(quality: f64) -> Vec<EvalRun> {
    (0..3)
        .map(|seed| EvalRun {
            task: "task".into(),
            family: "code".into(),
            seed,
            quality,
            tokens: 10,
            false_claims: 0,
            tamper: 0,
        })
        .collect()
}

fn evaluate(inc: &[EvalRun], cand: &[EvalRun]) -> crate::Result<Comparison> {
    compare(
        inc,
        cand,
        &Budget { max_tokens: 20 },
        &EvalPolicy::default(),
    )
}

#[test]
fn accepts_consistent_paired_improvement() {
    let result = evaluate(&runs(0.4), &runs(0.6)).unwrap();
    assert!(matches!(result.decision, EvalDecision::Accept { .. }));
    assert!((result.gain - 0.2).abs() < 1e-12);
    assert!(result.standard_error < 1e-12);
}

#[test]
fn overlapping_interval_is_inconclusive() {
    let mut candidate = runs(0.5);
    candidate[0].quality = 0.9;
    candidate[2].quality = 0.2;
    assert!(matches!(
        evaluate(&runs(0.5), &candidate).unwrap().decision,
        EvalDecision::Inconclusive { .. }
    ));
}

#[test]
fn rejects_clear_loser_and_health_or_cost_regression() {
    assert!(matches!(
        evaluate(&runs(0.6), &runs(0.4)).unwrap().decision,
        EvalDecision::Reject { .. }
    ));
    for field in 0..3 {
        let mut candidate = runs(0.8);
        match field {
            0 => candidate[0].false_claims = 1,
            1 => candidate[0].tamper = 1,
            _ => candidate[0].tokens = 11,
        }
        assert!(matches!(
            evaluate(&runs(0.4), &candidate).unwrap().decision,
            EvalDecision::Reject { .. }
        ));
    }
}

#[test]
fn invalid_matrix_and_data_are_errors() {
    let incumbent = runs(0.5);
    let mut cases = vec![vec![], incumbent[..1].to_vec(), incumbent[..2].to_vec()];
    let mut duplicate = incumbent.clone();
    duplicate.push(duplicate[0].clone());
    cases.push(duplicate);
    for case in cases {
        assert!(evaluate(&incumbent, &case).is_err());
    }
    for field in 0..5 {
        let mut candidate = runs(0.6);
        match field {
            0 => candidate[0].quality = f64::NAN,
            1 => candidate[0].quality = 1.1,
            2 => candidate[0].tokens = 21,
            3 => candidate[0].task = " ".into(),
            _ => candidate[0].family = " ".into(),
        }
        assert!(evaluate(&incumbent, &candidate).is_err());
    }
}

#[test]
fn family_weights_are_equal_and_seed_clustering_prevents_pseudoreplication() {
    let mut incumbent = runs(0.5);
    let mut candidate = runs(0.6);
    for task in ["other", "third"] {
        for seed in 0..3 {
            incumbent.push(EvalRun {
                task: task.into(),
                family: "research".into(),
                seed,
                ..runs(0.5)[0].clone()
            });
            candidate.push(EvalRun {
                task: task.into(),
                family: "research".into(),
                seed,
                ..runs(0.7)[0].clone()
            });
        }
    }
    let comparison = evaluate(&incumbent, &candidate).unwrap();
    assert!((comparison.gain - 0.15).abs() < 1e-12);
    candidate[3].quality = 0.4;
    candidate[6].quality = 0.4;
    let first = evaluate(&incumbent, &candidate).unwrap();
    let mut inc_many = incumbent.clone();
    let mut cand_many = candidate.clone();
    for index in 0..incumbent.len() {
        let mut i = incumbent[index].clone();
        i.task.push_str("copy");
        inc_many.push(i);
        let mut c = candidate[index].clone();
        c.task.push_str("copy");
        cand_many.push(c);
    }
    let repeated = evaluate(&inc_many, &cand_many).unwrap();
    assert!((first.standard_error - repeated.standard_error).abs() < 1e-12);
}

#[test]
fn rejects_family_loss_and_nonrectangular_seeds() {
    let mut incumbent = runs(0.5);
    let mut candidate = runs(0.9);
    for seed in 0..3 {
        incumbent.push(EvalRun {
            task: "research".into(),
            family: "research".into(),
            seed,
            ..runs(0.5)[0].clone()
        });
        candidate.push(EvalRun {
            task: "research".into(),
            family: "research".into(),
            seed,
            ..runs(0.4)[0].clone()
        });
    }
    assert!(matches!(
        evaluate(&incumbent, &candidate).unwrap().decision,
        EvalDecision::Reject { .. }
    ));
    incumbent.pop();
    candidate.pop();
    assert!(evaluate(&incumbent, &candidate).is_err());
}

#[test]
fn confirmation_requires_disjoint_tasks_and_seeds() {
    let selection = runs(0.5);
    assert!(ensure_independent(&selection, &selection).is_err());
    let mut confirmation = selection.clone();
    for run in &mut confirmation {
        run.task = "fresh".into();
    }
    assert!(ensure_independent(&selection, &confirmation).is_err());
    for run in &mut confirmation {
        run.seed += 10;
    }
    ensure_independent(&selection, &confirmation).unwrap();
    assert!(ensure_independent(&[], &confirmation).is_err());
}

#[test]
fn rejects_invalid_policy_and_zero_cap() {
    for value in [f64::NAN, -1.0, f64::INFINITY] {
        for field in 0..4 {
            let mut policy = EvalPolicy::default();
            match field {
                0 => policy.margin = value,
                1 => policy.standard_error_multiplier = value,
                2 => policy.family_regression_limit = value,
                _ => policy.cost_tolerance = value,
            }
            assert!(compare(&runs(0.5), &runs(0.6), &Budget { max_tokens: 20 }, &policy).is_err());
        }
    }
    assert!(
        compare(
            &runs(0.5),
            &runs(0.6),
            &Budget { max_tokens: 0 },
            &EvalPolicy::default()
        )
        .is_err()
    );
}

#[test]
fn run_and_decision_serde_representation_is_stable() {
    let run = &runs(0.5)[0];
    let value = serde_json::to_value(run).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"task":"task", "family":"code", "seed":0,
        "quality":0.5, "tokens":10, "false_claims":0, "tamper":0})
    );
    assert_eq!(serde_json::from_value::<EvalRun>(value).unwrap(), *run);
    let decision = EvalDecision::Accept {
        reason: "gain".into(),
    };
    assert_eq!(
        serde_json::to_value(&decision).unwrap(),
        serde_json::json!({"Accept":{"reason":"gain"}})
    );
}

#[test]
fn relative_cost_tolerance_and_existing_health_counts_are_respected() {
    let mut incumbent = runs(0.4);
    let mut candidate = runs(0.7);
    for run in &mut incumbent {
        run.false_claims = 2;
        run.tamper = 1;
    }
    for run in &mut candidate {
        run.tokens = 11;
        run.false_claims = 1;
    }
    let policy = EvalPolicy {
        cost_tolerance: 0.1,
        ..EvalPolicy::default()
    };
    assert!(matches!(
        compare(&incumbent, &candidate, &Budget { max_tokens: 20 }, &policy)
            .unwrap()
            .decision,
        EvalDecision::Accept { .. }
    ));
    for run in &mut incumbent {
        run.tokens = 0;
    }
    assert!(matches!(
        compare(&incumbent, &candidate, &Budget { max_tokens: 20 }, &policy)
            .unwrap()
            .decision,
        EvalDecision::Reject { .. }
    ));
}

#[test]
fn inconsistent_family_and_single_seed_matrices_are_invalid() {
    let incumbent = runs(0.5);
    let mut candidate = runs(0.6);
    candidate[0].family = "research".into();
    assert!(evaluate(&incumbent, &candidate).is_err());
    let mut duplicate_seed = runs(0.6);
    for run in &mut duplicate_seed {
        run.seed = 0;
    }
    assert!(evaluate(&duplicate_seed, &duplicate_seed).is_err());
    assert!(evaluate(&[], &candidate).is_err());
    let mut other_tasks = runs(0.6);
    for run in &mut other_tasks {
        run.task = "different".into();
    }
    assert!(evaluate(&incumbent, &other_tasks).is_err());
}

#[test]
fn policy_bounds_and_nonfinite_intervals_are_invalid() {
    for policy in [
        EvalPolicy {
            margin: 1.1,
            ..EvalPolicy::default()
        },
        EvalPolicy {
            family_regression_limit: 1.1,
            ..EvalPolicy::default()
        },
        EvalPolicy {
            standard_error_multiplier: 1.0,
            ..EvalPolicy::default()
        },
    ] {
        assert!(compare(&runs(0.5), &runs(0.6), &Budget { max_tokens: 20 }, &policy).is_err());
    }
    let mut candidate = runs(0.6);
    candidate[0].quality = -0.1;
    assert!(evaluate(&runs(0.5), &candidate).is_err());
    candidate[0].quality = f64::INFINITY;
    assert!(evaluate(&runs(0.5), &candidate).is_err());
}

#[test]
fn confirmation_rejects_blank_id_and_reused_task_with_fresh_seeds() {
    let selection = runs(0.5);
    let mut confirmation = selection.clone();
    for run in &mut confirmation {
        run.seed += 10;
    }
    assert!(ensure_independent(&selection, &confirmation).is_err());
    confirmation[0].task = " ".into();
    assert!(ensure_independent(&selection, &confirmation).is_err());
}

#[test]
fn zero_cost_tolerance_detects_single_token_above_float_precision() {
    let mut incumbent = runs(0.4);
    let mut candidate = runs(0.8);
    for run in &mut incumbent {
        run.tokens = 1_u64 << 54;
    }
    for run in &mut candidate {
        run.tokens = 1_u64 << 54;
    }
    candidate[0].tokens += 1;
    assert!(matches!(
        compare(
            &incumbent,
            &candidate,
            &Budget {
                max_tokens: u64::MAX
            },
            &EvalPolicy::default()
        )
        .unwrap()
        .decision,
        EvalDecision::Reject { .. }
    ));
}

#[test]
fn policy_budget_and_comparison_serde_roundtrip() {
    let budget = Budget { max_tokens: 20 };
    assert_eq!(
        serde_json::to_value(&budget).unwrap(),
        serde_json::json!({"max_tokens":20})
    );
    let policy = EvalPolicy::default();
    assert_eq!(
        serde_json::to_value(&policy).unwrap(),
        serde_json::json!({
        "margin":0.01, "standard_error_multiplier":2.0,
        "family_regression_limit":0.0, "cost_tolerance":0.0})
    );
    let comparison = evaluate(&runs(0.4), &runs(0.8)).unwrap();
    let encoded = serde_json::to_vec(&comparison).unwrap();
    assert_eq!(
        serde_json::from_slice::<Comparison>(&encoded).unwrap(),
        comparison
    );
}

#[test]
fn quality_uses_eval_owned_strengths_and_budget_capped_goal() {
    let rubric =
        QualityRubric::new(0.5, vec![VerifierTier::Proof, VerifierTier::Execution]).unwrap();
    let budget = Budget { max_tokens: 20 };
    let score = rubric.score(true, 20, &budget, &[true, false]).unwrap();
    assert!((score - (0.5 + 0.5 / 1.9)).abs() < 1e-12);
    let overrun = rubric.score(true, 21, &budget, &[true, false]).unwrap();
    assert!((overrun - 0.5 / 1.9).abs() < 1e-12);
    assert!(rubric.score(false, 20, &budget, &[true]).is_err());
    assert!(
        rubric
            .score(false, 20, &Budget { max_tokens: 0 }, &[true, true])
            .is_err()
    );
}

#[test]
fn judge_weights_are_earned_and_preferences_are_discounted() {
    let human = [true, true, false, false];
    let judge = [true, true, true, false];
    let calibration = JudgeCalibration::from_labels(&judge, &human, 0.4).unwrap();
    assert!((calibration.agreement() - 0.5).abs() < 1e-12);
    let rubric = QualityRubric::new(
        0.0,
        vec![
            VerifierTier::Proof,
            VerifierTier::RubricJudge(calibration.clone()),
            VerifierTier::Preference(calibration),
            VerifierTier::Schema,
            VerifierTier::HeldOutReference,
        ],
    )
    .unwrap();
    let score = rubric
        .score(
            false,
            0,
            &Budget { max_tokens: 20 },
            &[false, true, true, false, false],
        )
        .unwrap();
    assert!((score - 0.75 / 3.15).abs() < 1e-12);
    let unearned = JudgeCalibration::from_labels(&judge, &human, 0.6).unwrap();
    assert!(QualityRubric::new(0.5, vec![VerifierTier::RubricJudge(unearned)]).is_err());
    let worse = JudgeCalibration::from_labels(&[false, false, true, true], &human, 0.0).unwrap();
    assert!(QualityRubric::new(0.5, vec![VerifierTier::RubricJudge(worse)]).is_err());
}

#[test]
fn invalid_quality_rubric_or_calibration_is_rejected() {
    for alpha in [-1.0, 1.1, f64::NAN] {
        assert!(QualityRubric::new(alpha, vec![VerifierTier::Proof]).is_err());
    }
    assert!(QualityRubric::new(0.5, vec![]).is_err());
    for threshold in [-1.0, 1.1, f64::NAN] {
        assert!(JudgeCalibration::from_labels(&[true, false], &[true, false], threshold).is_err());
    }
    assert!(JudgeCalibration::from_labels(&[], &[], 0.5).is_err());
    assert!(JudgeCalibration::from_labels(&[true, false], &[true], 0.5).is_err());
    assert!(JudgeCalibration::from_labels(&[true, true], &[true, true], 0.5).is_err());
    let invalid: QualityRubric =
        serde_json::from_value(serde_json::json!({"alpha":2.0, "checkpoints":["Proof"]})).unwrap();
    assert!(
        invalid
            .score(true, 1, &Budget { max_tokens: 20 }, &[true])
            .is_err()
    );
    let invalid_calibration: JudgeCalibration =
        serde_json::from_value(serde_json::json!({"agreement":2.0,"threshold":0.0})).unwrap();
    assert!(QualityRubric::new(0.5, vec![VerifierTier::RubricJudge(invalid_calibration)]).is_err());
}

#[test]
fn frozen_quality_rubric_roundtrips_through_serde() {
    let rubric =
        QualityRubric::new(0.75, vec![VerifierTier::Proof, VerifierTier::Execution]).unwrap();
    let value = serde_json::to_value(&rubric).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"alpha":0.75,"checkpoints":["Proof","Execution"]})
    );
    assert_eq!(
        serde_json::from_value::<QualityRubric>(value).unwrap(),
        rubric
    );
}
