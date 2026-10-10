//! Failure paths retain evidence while protecting verifier and memory authority.

#![expect(clippy::unwrap_used, reason = "assert deterministic fixture outcomes")]

use super::test::{baseline, epoch, proposal};
use super::*;
use crate::{Budget, Package, Patch};
use tinyhivemind_lang::{Goal, MemoryIdentity, MemoryKind, MemoryLifecycle};

#[derive(Default)]
struct Host {
    watermarks: BTreeMap<String, u64>,
    pin_failure: bool,
    invalid_evidence: bool,
    noisy_confirmation: bool,
    stages: Vec<EvalStage>,
}

impl Evaluator for Host {
    fn pin_memory(&mut self, _: &CandidateState, _: &Patch) -> Result<BTreeMap<String, u64>> {
        if self.pin_failure {
            return Err(Error::HostFailed);
        }
        Ok(self.watermarks.clone())
    }
    fn evaluate(
        &mut self,
        _: &CandidateState,
        _: &CandidateState,
        stage: EvalStage,
        _: &Budget,
    ) -> Result<EvalPair> {
        self.stages.push(stage);
        let fresh = stage == EvalStage::Confirmation;
        let incumbent: Vec<_> = (0..3)
            .map(|seed| EvalRun {
                task: if fresh { "confirmation" } else { "selection" }.into(),
                family: "code".into(),
                seed: seed + if fresh { 10 } else { 0 },
                quality: 0.2,
                tokens: 10,
                false_claims: 0,
                tamper: 0,
            })
            .collect();
        let mut candidate = incumbent.clone();
        for run in &mut candidate {
            run.quality = if fresh && self.noisy_confirmation {
                match run.seed % 3 {
                    0 => 0.2,
                    1 => 0.4,
                    _ => 0.25,
                }
            } else {
                0.6
            };
        }
        if self.invalid_evidence {
            candidate.pop();
        }
        Ok(EvalPair {
            incumbent,
            candidate,
        })
    }
}

struct ForbiddenApproval;
impl ApprovalGate for ForbiddenApproval {
    fn approve(&mut self, _: &str, _: &Proposal, _: &Comparison) -> Result<bool> {
        Err(Error::HostFailed)
    }
}

fn state() -> CandidateState {
    let mut package = Package::default();
    package.constitution.permitted_edits =
        vec![PatchClass::Context, PatchClass::Memory, PatchClass::Goal];
    package.manifest.context = vec!["context/main.md".into()];
    package.documents.insert(
        "context/main.md".into(),
        "<!-- item:method -->\nGuess an answer.\n".into(),
    );
    package.memory.identities = vec![MemoryIdentity {
        id: "notes".into(),
        namespace: "team:hive/agent:notes".into(),
        kind: MemoryKind::Agent,
        lifecycle: MemoryLifecycle::Hive,
        read_only: false,
        fork_parent: None,
    }];
    CandidateState {
        package,
        memory_watermarks: BTreeMap::from([("notes".into(), 2)]),
    }
}

fn learning(watermark: u64) -> Proposal {
    let mut p = proposal();
    p.patch = Patch {
        class: PatchClass::Memory,
        operations: vec![Operation::Learning {
            identity: "notes".into(),
            entry: "e1".into(),
            watermark,
            before: None,
            after: Some("Verify before claiming success".into()),
        }],
    };
    p
}

#[test]
fn pinning_failures_and_unrelated_memory_changes_are_guarded() {
    let incumbent = state();
    assert!(matches!(
        prepare(
            &incumbent,
            &proposal(),
            &mut Host {
                pin_failure: true,
                ..Host::default()
            }
        ),
        Err(ReasonCode::HostFailure)
    ));
    assert!(matches!(
        prepare(
            &incumbent,
            &proposal(),
            &mut Host {
                watermarks: BTreeMap::from([("notes".into(), 3)]),
                ..Host::default()
            }
        ),
        Err(ReasonCode::Guard)
    ));
    let (candidate, _) = prepare(
        &incumbent,
        &proposal(),
        &mut Host {
            watermarks: incumbent.memory_watermarks.clone(),
            ..Host::default()
        },
    )
    .unwrap();
    assert_eq!(candidate.memory_watermarks, incumbent.memory_watermarks);
}

#[test]
fn learning_requires_matching_parent_and_a_strictly_advanced_host_watermark() {
    let incumbent = state();
    for (parent, next) in [(1, Some(3)), (2, None), (2, Some(2)), (2, Some(1))] {
        let watermarks = next.map_or_else(BTreeMap::new, |next| {
            BTreeMap::from([("notes".into(), next)])
        });
        assert!(matches!(
            prepare(
                &incumbent,
                &learning(parent),
                &mut Host {
                    watermarks,
                    ..Host::default()
                }
            ),
            Err(ReasonCode::Guard)
        ));
    }
    let (candidate, id) = prepare(
        &incumbent,
        &learning(2),
        &mut Host {
            watermarks: BTreeMap::from([("notes".into(), 3)]),
            ..Host::default()
        },
    )
    .unwrap();
    assert_eq!(candidate.memory_watermarks["notes"], 3);
    assert_ne!(
        id,
        lineage::content_id(&incumbent.package, &incumbent.memory_watermarks).unwrap()
    );
}

#[test]
fn goal_descriptions_can_change_but_host_acceptance_cannot_be_rewritten_or_removed() {
    let mut incumbent = state();
    incumbent.package.manifest.goals = vec![Goal {
        id: "solve".into(),
        description: "solve accurately".into(),
        acceptance: vec!["host test passes".into()],
        subgoals: vec![],
    }];
    for (pointer, before, after) in [
        (
            "/manifest/goals/0/acceptance",
            serde_json::json!(["host test passes"]),
            serde_json::json!(["self report"]),
        ),
        (
            "/manifest/goals",
            serde_json::to_value(&incumbent.package.manifest.goals).unwrap(),
            serde_json::json!([]),
        ),
    ] {
        let mut p = proposal();
        p.patch = Patch {
            class: PatchClass::Goal,
            operations: vec![Operation::Json {
                pointer: pointer.into(),
                before: Some(before),
                after: Some(after),
            }],
        };
        assert!(matches!(
            prepare(&incumbent, &p, &mut Host::default()),
            Err(ReasonCode::Guard)
        ));
    }
    let mut p = proposal();
    p.patch = Patch {
        class: PatchClass::Goal,
        operations: vec![Operation::Json {
            pointer: "/manifest/goals/0/description".into(),
            before: Some("solve accurately".into()),
            after: Some("prove the result".into()),
        }],
    };
    let (candidate, _) = prepare(
        &incumbent,
        &p,
        &mut Host {
            watermarks: incumbent.memory_watermarks.clone(),
            ..Host::default()
        },
    )
    .unwrap();
    assert_eq!(
        candidate.package.manifest.goals[0].acceptance,
        incumbent.package.manifest.goals[0].acceptance
    );
    assert_eq!(
        candidate.package.manifest.goals[0].description,
        "prove the result"
    );
}

#[test]
fn already_prepared_candidates_are_detected_before_spending_evaluation_budget() {
    let directory = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(directory.path().join("a.json")).unwrap();
    baseline(&mut archive);
    let head = archive.head().unwrap();
    let incumbent = CandidateState {
        package: archive.package(&head.id).unwrap().clone(),
        memory_watermarks: head.memory_watermarks.clone(),
    };
    let (candidate, id) = prepare(&incumbent, &proposal(), &mut Host::default()).unwrap();
    archive
        .prepare(candidate.package, candidate.memory_watermarks)
        .unwrap();
    let mut host = Host::default();
    let outcome = run_proposal(
        &mut archive,
        &epoch(),
        &proposal(),
        &mut host,
        &mut ForbiddenApproval,
    )
    .unwrap();
    assert_eq!(outcome.reason, ReasonCode::Duplicate);
    assert_eq!(outcome.candidate_id, Some(id));
    assert_eq!(host.stages, []);
    assert_eq!(archive.proposals().last().unwrap().reason, "Duplicate");
}

#[test]
fn invalid_paired_evidence_is_a_durable_host_failure_without_promotion() {
    let directory = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(directory.path().join("a.json")).unwrap();
    baseline(&mut archive);
    let old = archive.head().unwrap().id.clone();
    let outcome = run_proposal(
        &mut archive,
        &epoch(),
        &proposal(),
        &mut Host {
            invalid_evidence: true,
            ..Host::default()
        },
        &mut ForbiddenApproval,
    )
    .unwrap();
    assert_eq!(outcome.reason, ReasonCode::HostFailure);
    assert_eq!(archive.head().unwrap().id, old);
    assert!(outcome.comparison.is_none());
    assert_eq!(archive.evaluations().len(), 0);
    assert_eq!(archive.proposals().last().unwrap().reason, "HostFailure");
}

#[test]
fn uncertain_confirmation_keeps_the_incumbent_and_archives_rejection() {
    let directory = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(directory.path().join("a.json")).unwrap();
    baseline(&mut archive);
    let old = archive.head().unwrap().id.clone();
    let outcome = run_proposal(
        &mut archive,
        &epoch(),
        &proposal(),
        &mut Host {
            noisy_confirmation: true,
            ..Host::default()
        },
        &mut ForbiddenApproval,
    )
    .unwrap();
    assert_eq!(outcome.reason, ReasonCode::NotReplicated);
    assert!(matches!(
        outcome.comparison.unwrap().decision,
        EvalDecision::Inconclusive { .. }
    ));
    assert_eq!(archive.head().unwrap().id, old);
    assert_eq!(archive.rejected().len(), 1);
    assert_eq!(archive.evaluations().len(), 3);
}

#[test]
fn host_memory_snapshot_must_cover_exactly_the_declared_persistent_identities() {
    let incumbent = state();
    for watermarks in [
        BTreeMap::new(),
        BTreeMap::from([("notes".into(), 2), ("undeclared".into(), 1)]),
    ] {
        assert!(matches!(
            prepare(
                &incumbent,
                &proposal(),
                &mut Host {
                    watermarks,
                    ..Host::default()
                }
            ),
            Err(ReasonCode::Unformalizable)
        ));
    }
}
