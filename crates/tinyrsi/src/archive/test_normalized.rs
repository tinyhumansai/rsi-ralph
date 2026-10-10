//! Normalized evidence rejects forged decisions, policies, and approval bypasses.
#![expect(clippy::unwrap_used, reason = "assert deterministic archive fixtures")]
use super::*;
use crate::scoring::ScoringPolicySpec;
use crate::{
    Budget, EvalRun, FamilyAnchor, Operation, Patch, PatchClass, Prediction, Proposal, ScoreHealth,
    ScorePenalties,
};
use tinyhivemind_lang::lineage::{Direction, ObjectiveScore};

fn fixture() -> (
    tempfile::TempDir,
    Archive,
    NormalizedEvaluationEvent,
    Package,
) {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path().join("a.json")).unwrap();
    let mut original = Package::default();
    original.constitution.permitted_edits = vec![PatchClass::Context];
    original.manifest.context = vec!["context/main.md".into()];
    original.documents.insert(
        "context/main.md".into(),
        "<!-- item:method -->\nGuess\n".into(),
    );
    let patch = Patch {
        class: PatchClass::Context,
        operations: vec![Operation::Context {
            path: "context/main.md".into(),
            item: "method".into(),
            before: Some("Guess\n".into()),
            after: Some("Verify\n".into()),
        }],
    };
    let incumbent_id = a
        .record(
            original.clone(),
            VersionMetadata {
                parent: None,
                patch: Patch {
                    class: PatchClass::Context,
                    operations: vec![],
                },
                rationale: "baseline".into(),
                transcript_refs: vec![],
                scores: BTreeMap::from([(
                    "normalized_h".into(),
                    ObjectiveScore {
                        value: 0.0,
                        direction: Direction::Maximize,
                    },
                )]),
                verdict: Verdict::Accepted,
                memory_watermarks: BTreeMap::new(),
            },
        )
        .unwrap();
    let candidate = tinyhivemind_lang::patch::apply(&original, &patch)
        .unwrap()
        .candidate;
    let candidate_id =
        tinyhivemind_lang::lineage::content_id(&candidate, &BTreeMap::new()).unwrap();
    let proposal = Proposal {
        title: "verify".into(),
        patch,
        mechanism: "test before claims".into(),
        prediction: Prediction {
            metric: "normalized_h".into(),
            minimum_gain: 0.01,
            nodes: vec!["failed".into()],
        },
        transcript_refs: vec![1],
        risk: "time".into(),
        effort: "small".into(),
    };
    a.declare_proposal(ProposalDeclaration {
        epoch: 3,
        proposal: proposal.clone(),
    })
    .unwrap();
    let evidence = evidence(candidate_id, incumbent_id, proposal);
    a.declare_normalized_proposal(declaration(&evidence))
        .unwrap();
    a.prepare(candidate.clone(), BTreeMap::new()).unwrap();
    (dir, a, evidence, candidate)
}
fn evidence(
    candidate_id: String,
    incumbent_id: String,
    proposal: Proposal,
) -> NormalizedEvaluationEvent {
    let rows = |quality| {
        (0..3)
            .map(|seed| EvalRun {
                task: "task".into(),
                family: "code".into(),
                seed,
                quality,
                tokens: 1,
                false_claims: 0,
                tamper: 0,
            })
            .collect::<Vec<_>>()
    };
    let incumbent = rows(0.2);
    let candidate_runs = rows(0.4);
    let health = ScoreHealth {
        false_claim_rate: 0.0,
        tamper_rate: 0.0,
        cost_ratio: 1.0,
    };
    let policy = ScoringPolicySpec {
        anchors: vec![FamilyAnchor {
            family: "code".into(),
            baseline: 0.0,
            reference: 1.0,
            weight: 1.0,
            regression_limit: 0.0,
        }],
        penalties: ScorePenalties {
            false_claims: 1.0,
            tamper: 1.0,
            cost: 1.0,
        },
        margin: 0.01,
        cost_tolerance: 0.0,
        resamples: 200,
        bootstrap_seed: 0,
    };
    let budget = Budget { max_tokens: 10 };
    let comparison = crate::compare_h(
        &incumbent,
        &candidate_runs,
        &health,
        &health,
        &budget,
        &policy.build().unwrap(),
    )
    .unwrap();
    NormalizedEvaluationEvent {
        candidate_id,
        incumbent_id,
        epoch: 3,
        proposal,
        stage: "Screen".into(),
        suite_id: "s".into(),
        suite_ids: BTreeMap::from([
            ("Screen".into(), "s".into()),
            ("Selection".into(), "v".into()),
            ("Confirmation".into(), "c".into()),
        ]),
        incumbent,
        candidate: candidate_runs,
        incumbent_health: health.clone(),
        candidate_health: health,
        budget,
        policy,
        comparison,
    }
}
fn stages(a: &mut Archive, screen: &NormalizedEvaluationEvent) -> NormalizedEvaluationEvent {
    a.record_normalized_evaluation(screen.clone()).unwrap();
    let mut selection = screen.clone();
    selection.stage = "Selection".into();
    selection.suite_id = "v".into();
    a.record_normalized_evaluation(selection).unwrap();
    let mut confirmation = screen.clone();
    confirmation.stage = "Confirmation".into();
    confirmation.suite_id = "c".into();
    for row in confirmation
        .incumbent
        .iter_mut()
        .chain(confirmation.candidate.iter_mut())
    {
        row.task = "fresh".into();
        row.seed += 10;
    }
    a.record_normalized_evaluation(confirmation.clone())
        .unwrap();
    confirmation
}
fn metadata(e: &NormalizedEvaluationEvent) -> VersionMetadata {
    VersionMetadata {
        parent: Some(e.incumbent_id.clone()),
        patch: e.proposal.patch.clone(),
        rationale: "normalized result".into(),
        transcript_refs: vec![1],
        scores: BTreeMap::from([(
            "normalized_h".into(),
            ObjectiveScore {
                value: e.comparison.candidate.value,
                direction: Direction::Maximize,
            },
        )]),
        verdict: Verdict::Accepted,
        memory_watermarks: BTreeMap::new(),
    }
}
#[test]
fn normalized_event_validates_all_bound_configuration_and_measurements() {
    for field in 0..13 {
        let (_dir, mut a, e, _) = fixture();
        let mut bad = e;
        match field {
            0 => bad.stage = "anchor".into(),
            1 => bad.incumbent_id = "other".into(),
            2 => bad.candidate_id = "other".into(),
            3 => {
                bad.suite_ids.remove("Screen");
            }
            4 => bad
                .suite_ids
                .insert("Screen".into(), String::new())
                .map(drop)
                .unwrap(),
            5 => bad.suite_id = "different".into(),
            6 => bad.proposal.prediction.metric = "verified_quality".into(),
            7 => bad.proposal.prediction.minimum_gain = 0.0,
            8 => bad.policy.margin = 0.0,
            9 => bad.epoch = 999,
            10 => bad.policy.resamples = 199,
            11 => bad.comparison.delta += 0.1,
            _ => bad.candidate.pop().map(drop).unwrap(),
        }
        assert_eq!(
            a.record_normalized_evaluation(bad),
            Err(Error::InvalidRecord)
        );
        assert_eq!(a.normalized_evaluations().len(), 0);
    }
}
#[test]
fn stage_sequence_and_frozen_context_cannot_be_changed_after_screening() {
    for field in 0..6 {
        let (_dir, mut a, e, _) = fixture();
        a.record_normalized_evaluation(e.clone()).unwrap();
        let mut bad = e;
        bad.stage = "Selection".into();
        bad.suite_id = "v".into();
        match field {
            0 => {
                bad.stage = "Screen".into();
                bad.suite_id = "s".into();
            }
            1 => bad.policy.bootstrap_seed = 99,
            2 => bad.budget.max_tokens += 1,
            3 => bad
                .suite_ids
                .insert("Confirmation".into(), "changed".into())
                .map(drop)
                .unwrap(),
            4 => {
                bad.proposal.risk = "changed".into();
                a.declare_proposal(ProposalDeclaration {
                    epoch: bad.epoch,
                    proposal: bad.proposal.clone(),
                })
                .unwrap();
            }
            _ => {
                bad.epoch += 1;
                a.declare_proposal(ProposalDeclaration {
                    epoch: bad.epoch,
                    proposal: bad.proposal.clone(),
                })
                .unwrap();
            }
        }
        assert_eq!(
            a.record_normalized_evaluation(bad),
            Err(Error::InvalidRecord)
        );
    }
}
#[test]
fn normalized_accepted_record_cannot_bypass_human_approval() {
    let (_dir, mut a, e, package) = fixture();
    let confirmation = stages(&mut a, &e);
    assert_eq!(
        a.record(package.clone(), metadata(&confirmation)),
        Err(Error::InvalidRecord)
    );
    a.record_normalized_approval(NormalizedApprovalEvent {
        candidate_id: e.candidate_id.clone(),
        comparison: confirmation.comparison.clone(),
        approved: false,
    })
    .unwrap();
    assert_eq!(
        a.record(package, metadata(&confirmation)),
        Err(Error::InvalidRecord)
    );
    assert_eq!(a.normalized_approvals().len(), 1);
}
#[test]
fn approval_binds_exact_positive_confirmation_and_is_not_repeatable() {
    let (_dir, mut a, e, _) = fixture();
    let approval = NormalizedApprovalEvent {
        candidate_id: e.candidate_id.clone(),
        comparison: e.comparison.clone(),
        approved: true,
    };
    assert_eq!(
        a.record_normalized_approval(approval.clone()),
        Err(Error::InvalidRecord)
    );
    let confirmation = stages(&mut a, &e);
    let mut altered = approval.clone();
    altered.comparison.delta += 1.0;
    assert_eq!(
        a.record_normalized_approval(altered),
        Err(Error::InvalidRecord)
    );
    let approval = NormalizedApprovalEvent {
        comparison: confirmation.comparison,
        ..approval
    };
    a.record_normalized_approval(approval.clone()).unwrap();
    assert_eq!(
        a.record_normalized_approval(approval),
        Err(Error::InvalidRecord)
    );
}
#[test]
fn normalized_record_objective_parent_and_patch_are_exact() {
    for field in 0..4 {
        let (_dir, mut a, e, package) = fixture();
        let confirmation = stages(&mut a, &e);
        a.record_normalized_approval(NormalizedApprovalEvent {
            candidate_id: e.candidate_id.clone(),
            comparison: confirmation.comparison.clone(),
            approved: true,
        })
        .unwrap();
        let mut meta = metadata(&confirmation);
        match field {
            0 => meta.scores.clear(),
            1 => meta.scores.get_mut("normalized_h").unwrap().value += 0.1,
            2 => meta.parent = Some("other".into()),
            _ => meta.scores.get_mut("normalized_h").unwrap().direction = Direction::Minimize,
        }
        assert_eq!(a.record(package, meta), Err(Error::InvalidRecord));
    }
}
#[test]
fn normalized_archive_replays_exact_decisions_in_schema_two() {
    let (dir, mut a, e, package) = fixture();
    let confirmation = stages(&mut a, &e);
    a.record_normalized_approval(NormalizedApprovalEvent {
        candidate_id: e.candidate_id.clone(),
        comparison: confirmation.comparison.clone(),
        approved: true,
    })
    .unwrap();
    a.record(package, metadata(&confirmation)).unwrap();
    drop(a);
    let path = dir.path().join("a.json");
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["version"], 2);
    let a = Archive::open(path).unwrap();
    assert_eq!(a.normalized_evaluations().len(), 3);
    assert_eq!(a.normalized_approvals().len(), 1);
    assert_eq!(a.head().unwrap().id, e.candidate_id);
}

#[test]
fn rejected_and_inconclusive_stages_cannot_advance_the_funnel() {
    for uncertain in [false, true] {
        let (_dir, mut archive, mut screen, _) = fixture();
        if !uncertain {
            for run in &mut screen.candidate {
                run.quality = 0.1;
            }
            screen.comparison = crate::compare_h(
                &screen.incumbent,
                &screen.candidate,
                &screen.incumbent_health,
                &screen.candidate_health,
                &screen.budget,
                &screen.policy.build().unwrap(),
            )
            .unwrap();
        }
        archive
            .record_normalized_evaluation(screen.clone())
            .unwrap();
        let mut next = screen.clone();
        next.stage = "Selection".into();
        next.suite_id = "v".into();
        if uncertain {
            for (run, quality) in next.candidate.iter_mut().zip([0.2, 0.4, 0.25]) {
                run.quality = quality;
            }
            next.comparison = crate::compare_h(
                &next.incumbent,
                &next.candidate,
                &next.incumbent_health,
                &next.candidate_health,
                &next.budget,
                &next.policy.build().unwrap(),
            )
            .unwrap();
            assert!(matches!(
                next.comparison.decision,
                crate::EvalDecision::Inconclusive { .. }
            ));
            archive.record_normalized_evaluation(next.clone()).unwrap();
            next.stage = "Confirmation".into();
            next.suite_id = "c".into();
        }
        assert_eq!(
            archive.record_normalized_evaluation(next),
            Err(Error::InvalidRecord)
        );
    }
}

#[test]
fn reused_confirmation_cannot_receive_normalized_human_approval() {
    let (_dir, mut archive, screen, _) = fixture();
    archive
        .record_normalized_evaluation(screen.clone())
        .unwrap();
    let mut next = screen.clone();
    next.stage = "Selection".into();
    next.suite_id = "v".into();
    archive.record_normalized_evaluation(next.clone()).unwrap();
    next.stage = "Confirmation".into();
    next.suite_id = "c".into();
    archive.record_normalized_evaluation(next.clone()).unwrap();
    assert_eq!(
        archive.record_normalized_approval(NormalizedApprovalEvent {
            candidate_id: screen.candidate_id,
            comparison: next.comparison,
            approved: true
        }),
        Err(Error::InvalidRecord)
    );
}

#[test]
fn normalized_evidence_must_describe_the_declared_patch_result() {
    let (_dir, mut archive, mut evidence, _) = fixture();
    if let Operation::Context { after, .. } = &mut evidence.proposal.patch.operations[0] {
        *after = Some("Unrelated package\n".into());
    }
    archive
        .declare_proposal(ProposalDeclaration {
            epoch: evidence.epoch,
            proposal: evidence.proposal.clone(),
        })
        .unwrap();
    assert_eq!(
        archive.record_normalized_evaluation(evidence),
        Err(Error::InvalidRecord)
    );
}

fn declaration(e: &NormalizedEvaluationEvent) -> NormalizedDeclarationEvent {
    NormalizedDeclarationEvent {
        epoch: e.epoch,
        proposal: e.proposal.clone(),
        incumbent_id: e.incumbent_id.clone(),
        policy: e.policy.clone(),
        budget: e.budget.clone(),
        suite_ids: e.suite_ids.clone(),
    }
}

#[test]
fn normalized_policy_and_suites_must_be_declared_before_candidate_outputs() {
    let (_dir, a, e, _) = fixture();
    let path = a.path.clone();
    let mut snapshot: Snapshot = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    snapshot
        .events
        .retain(|event| !matches!(event, Event::NormalizedDeclaration { .. }));
    std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    drop(a);
    let mut a = Archive::open(&path).unwrap();
    assert_eq!(
        a.record_normalized_evaluation(e.clone()),
        Err(Error::InvalidRecord)
    );
    assert_eq!(
        a.declare_normalized_proposal(declaration(&e)),
        Err(Error::InvalidRecord)
    );
}

#[test]
fn normalized_declarations_reject_epoch_contradictions_and_retroactive_rebinding() {
    let (_dir, a, e, _) = fixture();
    let path = a.path.clone();
    let mut snapshot: Snapshot = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    snapshot.events.retain(|event| {
        !matches!(
            event,
            Event::Prepared { .. } | Event::NormalizedDeclaration { .. }
        )
    });
    std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    drop(a);
    let mut a = Archive::open(&path).unwrap();
    let d = declaration(&e);
    a.declare_normalized_proposal(d.clone()).unwrap();
    assert_eq!(a.normalized_declarations(), vec![&d]);
    assert_eq!(
        a.declare_normalized_proposal(d.clone()),
        Err(Error::InvalidRecord)
    );
    for change in [0, 1, 2] {
        let mut contradiction = d.clone();
        contradiction.proposal.title.push('x');
        match change {
            0 => contradiction.policy.bootstrap_seed += 1,
            1 => contradiction.budget.max_tokens += 1,
            _ => {
                contradiction
                    .suite_ids
                    .insert("Screen".into(), "other".into());
            }
        }
        a.declare_proposal(ProposalDeclaration {
            epoch: contradiction.epoch,
            proposal: contradiction.proposal.clone(),
        })
        .unwrap();
        assert_eq!(
            a.declare_normalized_proposal(contradiction),
            Err(Error::InvalidRecord)
        );
    }
    let mut next = d;
    next.epoch += 1;
    next.proposal.title.push('y');
    a.declare_proposal(ProposalDeclaration {
        epoch: next.epoch,
        proposal: next.proposal.clone(),
    })
    .unwrap();
    a.declare_normalized_proposal(next).unwrap();
}

#[test]
fn a_preregistered_normalized_candidate_cannot_skip_all_evaluation_evidence() {
    let (_dir, mut a, e, package) = fixture();
    assert_eq!(a.record(package, metadata(&e)), Err(Error::InvalidRecord));
}

#[test]
fn a_declared_candidate_cannot_bypass_preparation_and_normalized_evidence() {
    let (_dir, a, e, package) = fixture();
    let path = a.path.clone();
    let mut snapshot: Snapshot = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    snapshot
        .events
        .retain(|event| !matches!(event, Event::Prepared { .. }));
    std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    drop(a);
    let mut a = Archive::open(&path).unwrap();
    assert_eq!(a.record(package, metadata(&e)), Err(Error::InvalidRecord));
}

fn before_normalized_outputs(a: Archive) -> Archive {
    let path = a.path.clone();
    let mut snapshot: Snapshot = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    snapshot.events.retain(|event| {
        !matches!(
            event,
            Event::Prepared { .. } | Event::NormalizedDeclaration { .. }
        )
    });
    std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    drop(a);
    Archive::open(&path).unwrap()
}

#[test]
fn invalid_normalized_preregistration_never_creates_a_frozen_event() {
    for change in 0..9 {
        let (_dir, a, e, _) = fixture();
        let mut a = before_normalized_outputs(a);
        let mut d = declaration(&e);
        match change {
            0 => d.budget.max_tokens = 0,
            1 => d.policy.resamples = 0,
            2 => {
                d.suite_ids.remove("Screen");
            }
            3 => {
                d.suite_ids.insert("Screen".into(), String::new());
            }
            4 => {
                d.suite_ids
                    .insert("Screen".into(), d.suite_ids["Selection"].clone());
            }
            5 => d.incumbent_id = "unknown".into(),
            6 => d.proposal.prediction.metric = "verified_quality".into(),
            7 => d.proposal.prediction.minimum_gain = 0.,
            _ => d.proposal.title = "not declared".into(),
        }
        assert_eq!(a.declare_normalized_proposal(d), Err(Error::InvalidRecord));
        assert_eq!(
            a.normalized_declarations(),
            Vec::<&NormalizedDeclarationEvent>::new()
        );
    }
}

#[test]
fn host_failure_outputs_cannot_be_retrofitted_with_normalized_preregistration() {
    let (_dir, a, e, _) = fixture();
    let mut a = before_normalized_outputs(a);
    a.record_proposal(ProposalEvent {
        epoch: e.epoch,
        patch: e.proposal.patch.clone(),
        rationale: "attempt".into(),
        reason: "host_failure".into(),
        transcript_refs: vec![],
    })
    .unwrap();
    assert_eq!(
        a.declare_normalized_proposal(declaration(&e)),
        Err(Error::InvalidRecord)
    );
}
