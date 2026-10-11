//! Private suite freezing, coordinator verification and paired isolation.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::{ModelFuture, ModelReply, ModelRequest};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
struct Model {
    calls: AtomicUsize,
    bad: bool,
}
impl ModelPort for Model {
    fn generate(&self, _: ModelRequest) -> ModelFuture {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let bad = self.bad;
        Box::pin(async move {
            Ok(ModelReply {
                content: if bad {
                    b"bad".to_vec()
                } else {
                    b"good".to_vec()
                },
                input_tokens: 10,
                output_tokens: 2,
            })
        })
    }
}
fn task(stage: &str, seed: u64) -> PrivateTask {
    PrivateTask::new(
        stage.into(),
        "code".into(),
        "alice".into(),
        format!("Solve {stage} private canary"),
        crate::HostVerifier::Golden {
            expected: b"good".to_vec(),
        },
        vec![seed, seed + 1],
    )
    .unwrap()
}
fn suites() -> PrivateSuites {
    PrivateSuites::freeze(
        vec![task("screen", 1)],
        vec![task("selection", 3)],
        vec![task("confirmation", 5)],
        vec![task("anchor", 7)],
    )
    .unwrap()
}
fn state() -> CandidateState {
    let mut package = crate::Package::default();
    package.manifest.roles.push(tinyhivemind_lang::Role {
        id: "builder".into(),
        charter: "roles/builder.md".into(),
    });
    package
        .documents
        .insert("roles/builder.md".into(), "Build correct artifacts".into());
    package
        .documents
        .insert("seats/alice.md".into(), "Solve carefully".into());
    package.manifest.seats.push(tinyhivemind_lang::Seat {
        id: "alice".into(),
        label: "Alice".into(),
        role: "builder".into(),
        prompt: "seats/alice.md".into(),
        template: "agent".into(),
        config: serde_json::json!({}),
        ..Default::default()
    });
    CandidateState {
        package,
        memory_watermarks: BTreeMap::new(),
    }
}
#[test]
fn freezes_four_disjoint_stages_without_exposing_private_inputs() {
    let suite = suites();
    let hashes = suite.fingerprints();
    assert_eq!(hashes.len(), 4);
    assert_eq!(hashes.values().collect::<BTreeSet<_>>().len(), 4);
    assert!(!format!("{suite:?}").contains("private canary"));
    let bad = PrivateSuites::freeze(
        vec![task("screen", 1)],
        vec![task("screen", 3)],
        vec![task("confirmation", 5)],
        vec![task("anchor", 7)],
    );
    assert!(bad.is_err());
}
#[test]
fn actual_coordinator_verification_scores_complete_paired_seed_runs() {
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        bad: false,
    });
    let mut evaluator =
        CoordinatorEvaluator::new(suites(), model.clone(), memory, 100, 1000).unwrap();
    let state = state();
    let result = evaluator
        .evaluate_h(
            &state,
            &state,
            EvalStage::Selection,
            &Budget { max_tokens: 10_000 },
        )
        .unwrap();
    assert_eq!(result.pair.incumbent.len(), 2);
    assert!(
        result
            .pair
            .incumbent
            .iter()
            .all(|r| (r.quality - 1.0).abs() < f64::EPSILON && r.tokens == 12)
    );
    assert_eq!(result.pair.incumbent, result.pair.candidate);
    assert_eq!(model.calls.load(Ordering::SeqCst), 4);
    assert_eq!(result.candidate_health.false_claim_rate, 0.0);
    assert_eq!(result.candidate_health.cost_ratio, 1.0);
}
#[test]
fn failed_host_verifier_awards_zero_credit_and_measures_false_claim_denominator() {
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let mut evaluator = CoordinatorEvaluator::new(
        suites(),
        Arc::new(Model {
            calls: AtomicUsize::new(0),
            bad: true,
        }),
        memory,
        100,
        1000,
    )
    .unwrap();
    let state = state();
    let result = evaluator
        .evaluate_h(
            &state,
            &state,
            EvalStage::Screen,
            &Budget { max_tokens: 10_000 },
        )
        .unwrap();
    assert!(
        result
            .pair
            .candidate
            .iter()
            .all(|r| r.quality == 0.0 && r.false_claims == 1)
    );
    assert_eq!(result.candidate_health.false_claim_rate, 1.0);
}
#[test]
fn unsupported_capabilities_and_small_caps_are_rejected_before_model_calls() {
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        bad: false,
    });
    let mut evaluator =
        CoordinatorEvaluator::new(suites(), model.clone(), memory, 100, 1000).unwrap();
    let mut state = state();
    state.package.manifest.seats[0].mcp.push("external".into());
    assert!(
        evaluator
            .evaluate_h(
                &state,
                &state,
                EvalStage::Screen,
                &Budget { max_tokens: 10_000 }
            )
            .is_err()
    );
    state.package.manifest.seats[0].mcp.clear();
    assert!(
        evaluator
            .evaluate_h(&state, &state, EvalStage::Screen, &Budget { max_tokens: 1 })
            .is_err()
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn frozen_anchor_subdraws_keep_protocol_identity_and_use_only_requested_seeds() {
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        bad: false,
    });
    let mut evaluator =
        CoordinatorEvaluator::new(suites(), model.clone(), memory, 100, 1000).unwrap();
    let id = evaluator.anchor_suite_id();
    let (runs, health) = evaluator
        .evaluate_anchor_seeds(&state(), &Budget { max_tokens: 10_000 }, &[7])
        .unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].seed, 7);
    assert_eq!(health.cost_ratio, 1.0);
    assert_eq!(model.calls.load(Ordering::SeqCst), 1);
    assert_eq!(evaluator.anchor_suite_id(), id);
    assert!(
        evaluator
            .evaluate_anchor_seeds(&state(), &Budget { max_tokens: 10_000 }, &[9])
            .is_err()
    );
    assert!(
        evaluator
            .evaluate_anchor_seeds(&state(), &Budget { max_tokens: 10_000 }, &[7, 7])
            .is_err()
    );
    let (runs, _) = evaluator
        .evaluate_anchor_once(&state(), &Budget { max_tokens: 10_000 })
        .unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(
        evaluator.suite_ids().keys().cloned().collect::<Vec<_>>(),
        vec!["Confirmation", "Screen", "Selection"]
    );
}
#[test]
fn pinned_learning_maps_are_isolated_and_do_not_change_deployed_memory() {
    struct Recall;
    impl ModelPort for Recall {
        fn generate(&self, request: ModelRequest) -> ModelFuture {
            let good = request.system().contains("method: improved");
            Box::pin(async move {
                Ok(ModelReply {
                    content: if good {
                        b"good".to_vec()
                    } else {
                        b"bad".to_vec()
                    },
                    input_tokens: 10,
                    output_tokens: 2,
                })
            })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(
        dir.path().join("memory"),
        vec!["private canary".into()],
        100_000,
    )
    .unwrap();
    let incumbent = recalled_state();
    memory
        .seed(
            &incumbent.package,
            &incumbent.memory_watermarks,
            BTreeMap::from([(
                "notes".into(),
                BTreeMap::from([("method".into(), "old".into())]),
            )]),
        )
        .unwrap();
    memory
        .activate(&incumbent.package, &incumbent.memory_watermarks)
        .unwrap();
    let patch = crate::Patch {
        class: crate::PatchClass::Memory,
        operations: vec![crate::Operation::Learning {
            identity: "notes".into(),
            entry: "method".into(),
            watermark: 1,
            before: Some("old".into()),
            after: Some("improved".into()),
        }],
    };
    let mut evaluator =
        CoordinatorEvaluator::new(suites(), Arc::new(Recall), memory.clone(), 100, 1000).unwrap();
    let marks = evaluator.pin_memory(&incumbent, &patch).unwrap();
    let candidate = CandidateState {
        package: tinyhivemind_lang::patch::apply(&incumbent.package, &patch)
            .unwrap()
            .candidate,
        memory_watermarks: marks,
    };
    let result = evaluator
        .evaluate_h(
            &incumbent,
            &candidate,
            EvalStage::Confirmation,
            &Budget { max_tokens: 10_000 },
        )
        .unwrap();
    assert!(result.pair.incumbent.iter().all(|r| r.quality == 0.0));
    assert!(
        result
            .pair
            .candidate
            .iter()
            .all(|r| (r.quality - 1.0).abs() < f64::EPSILON)
    );
    assert_eq!(
        memory.active_watermarks().unwrap(),
        incumbent.memory_watermarks
    );
    assert_eq!(
        memory
            .read(&incumbent.package, &incumbent.memory_watermarks)
            .unwrap()["notes"]["method"],
        "old"
    );
}
#[tokio::test]
async fn controlled_factory_uses_real_verifier_completion_and_external_usage_meter() {
    use tinyhivemind_hives::{Destination, HiveInfo, MemoryStorage, SendMessage};
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        bad: false,
    });
    let factory = Arc::new(
        ControlledRunnerFactory::new(
            model,
            memory.clone(),
            task("production", 99),
            Budget { max_tokens: 10_000 },
            100,
            1000,
        )
        .unwrap(),
    );
    let mut host = crate::CoordinatorHost::new(
        "production".into(),
        Arc::new(MemoryStorage::new()),
        factory.clone(),
        Arc::new(memory),
    );
    crate::ActivationBackend::apply(&mut host, &state().package, &BTreeMap::new())
        .await
        .unwrap();
    let coordinator = host.coordinator().unwrap();
    coordinator
        .create_hive(HiveInfo {
            hive_id: "work".into(),
            name: "work".into(),
            description: None,
            members: vec!["alice".into()],
        })
        .await
        .unwrap();
    coordinator
        .send_as_host(SendMessage {
            message_id: "work".into(),
            sender: String::new(),
            destination: Destination::Hive("work".into()),
            body: "Produce the requested artifact".into(),
            thread: None,
            only_for: vec![],
            starters: vec![],
        })
        .await
        .unwrap();
    coordinator.run_until_idle().await.unwrap();
    assert!(factory.evidence().unwrap()[0].passed);
    assert_eq!(factory.usage().unwrap()[0].input_tokens, 10);
    assert_eq!(factory.usage().unwrap()[0].output_tokens, 2);
    assert!(!format!("{factory:?}").contains("private canary"));
}

#[path = "test_attribution.rs"]
mod attribution;
#[path = "test_journal.rs"]
mod journal;
#[path = "test_private.rs"]
mod private;

fn recalled_state() -> CandidateState {
    use tinyhivemind_lang::{
        MemoryIdentity, MemoryKind, MemoryLifecycle, MemoryReach, RecallMoment, SeatMemory,
    };
    let mut incumbent = state();
    incumbent
        .package
        .constitution
        .permitted_edits
        .push(crate::PatchClass::Memory);
    incumbent.package.memory.identities.push(MemoryIdentity {
        id: "notes".into(),
        namespace: "team:hive/agent:alice".into(),
        kind: MemoryKind::Agent,
        lifecycle: MemoryLifecycle::Hive,
        read_only: false,
        fork_parent: None,
    });
    incumbent.package.memory.bindings.push(SeatMemory {
        seat: "alice".into(),
        identity: "notes".into(),
        reads: vec![MemoryReach {
            at: "team:hive/agent:alice".into(),
            inherit: false,
            descendants: false,
        }],
        recall_at: vec![RecallMoment::SessionStart],
        budget_chars: 100,
        remember: vec![],
    });
    incumbent.memory_watermarks.insert("notes".into(), 1);
    incumbent
}
