//! Durable exact proposal inputs and evaluation policy evidence.
#![expect(clippy::unwrap_used, reason = "assert test setup and outcomes")]
use super::test::baseline;
use super::*;

#[test]
fn persists_exact_proposal_prediction_risk_and_effort_before_evaluation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let proposal = serde_json::from_value(serde_json::json!({
        "title":"Check dependencies", "patch":{"class":"prompt","operations":[]},
        "mechanism":"Verify before completing", "risk":"extra tokens", "effort":"one turn",
        "prediction":{"metric":"verified_quality","minimum_gain":0.12,"nodes":["n1"]},
        "transcript_refs":[3,5]
    }))
    .unwrap();
    let declaration = ProposalDeclaration { epoch: 7, proposal };
    archive.declare_proposal(declaration.clone()).unwrap();
    assert_eq!(archive.records().len(), 0);
    drop(archive);
    let archive = Archive::open(&path).unwrap();
    assert_eq!(archive.declarations(), vec![&declaration]);
}

#[test]
fn rejects_nonfinite_declared_predictions_without_persisting_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("archive.json")).unwrap();
    let proposal = serde_json::from_value(serde_json::json!({
        "title":"Check", "patch":{"class":"context","operations":[]},
        "mechanism":"Verify", "risk":"extra tokens", "effort":"one turn",
        "prediction":{"metric":"verified_quality","minimum_gain":0.12,"nodes":["n1"]},
        "transcript_refs":[3]
    }))
    .unwrap();
    let mut declaration = ProposalDeclaration { epoch: 7, proposal };
    declaration.proposal.prediction.minimum_gain = f64::NAN;
    assert_eq!(
        archive.declare_proposal(declaration),
        Err(Error::InvalidRecord)
    );
    assert_eq!(archive.declarations(), Vec::<&ProposalDeclaration>::new());
}

#[test]
fn persists_exact_paired_evidence_and_rejects_invalid_runs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let base = baseline(&mut archive);
    let run = crate::EvalRun {
        task: "task".into(),
        family: "code".into(),
        seed: 1,
        quality: 0.5,
        tokens: 5,
        false_claims: 0,
        tamper: 0,
    };
    let event = EvaluationEvent {
        candidate_id: base,
        stage: "confirmation".into(),
        incumbent: vec![run.clone()],
        candidate: vec![run],
        budget: None,
        policy: None,
        comparison: None,
    };
    archive.record_evaluation(event.clone()).unwrap();
    let mut invalid = event.clone();
    invalid.candidate[0].quality = f64::NAN;
    assert_eq!(
        archive.record_evaluation(invalid),
        Err(Error::InvalidRecord)
    );
    let mut invalid = event.clone();
    invalid.candidate[0].seed += 1;
    assert_eq!(
        archive.record_evaluation(invalid),
        Err(Error::InvalidRecord)
    );
    let mut invalid = event.clone();
    invalid.incumbent.push(invalid.incumbent[0].clone());
    assert_eq!(
        archive.record_evaluation(invalid),
        Err(Error::InvalidRecord)
    );
    let mut invalid = event.clone();
    invalid.candidate_id = "invalid".into();
    assert_eq!(
        archive.record_evaluation(invalid),
        Err(Error::InvalidRecord)
    );
    drop(archive);
    let archive = Archive::open(path).unwrap();
    assert_eq!(archive.evaluations(), vec![&event]);
}

#[test]
fn replays_exact_policy_and_comparison_and_rejects_forged_summary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let base = baseline(&mut archive);
    let budget = crate::Budget { max_tokens: 100 };
    let policy = crate::EvalPolicy::default();
    let incumbent: Vec<_> = [1, 2, 3]
        .into_iter()
        .map(|seed| crate::EvalRun {
            task: "task".into(),
            family: "code".into(),
            seed,
            quality: 0.123_456_789,
            tokens: 10,
            false_claims: 0,
            tamper: 0,
        })
        .collect();
    let mut candidate = incumbent.clone();
    for (run, quality) in candidate.iter_mut().zip([0.3, 0.5, 0.7]) {
        run.quality = quality;
    }
    let comparison = crate::compare(&incumbent, &candidate, &budget, &policy).unwrap();
    let event = EvaluationEvent {
        candidate_id: base,
        stage: "selection".into(),
        incumbent,
        candidate,
        budget: Some(budget),
        policy: Some(policy),
        comparison: Some(comparison),
    };
    archive.record_evaluation(event.clone()).unwrap();
    let mut invalid = event.clone();
    invalid.policy = None;
    assert_eq!(
        archive.record_evaluation(invalid),
        Err(Error::InvalidRecord)
    );
    let mut invalid = event.clone();
    invalid.comparison.as_mut().unwrap().gain += 0.01;
    assert_eq!(
        archive.record_evaluation(invalid),
        Err(Error::InvalidRecord)
    );
    drop(archive);
    let archive = Archive::open(path).unwrap();
    assert_eq!(archive.evaluations(), vec![&event]);
}
