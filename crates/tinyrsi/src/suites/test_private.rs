//! Frozen checker identity, secret exclusion and failure-path regressions.
use super::*;
#[test]
fn task_constructor_and_freezing_reject_ambiguous_private_draws() {
    let verifier = crate::HostVerifier::Golden {
        expected: b"good".to_vec(),
    };
    assert!(
        PrivateTask::new(
            String::new(),
            "family".into(),
            "alice".into(),
            "task".into(),
            verifier.clone(),
            vec![1]
        )
        .is_err()
    );
    assert!(
        PrivateTask::new(
            "id".into(),
            "family".into(),
            "alice".into(),
            "task".into(),
            verifier.clone(),
            vec![1, 1]
        )
        .is_err()
    );
    assert!(
        PrivateTask::new(
            "id".into(),
            "family".into(),
            "alice".into(),
            "task".into(),
            verifier,
            vec![]
        )
        .is_err()
    );
    assert!(
        PrivateSuites::freeze(
            vec![],
            vec![task("selection", 3)],
            vec![task("confirmation", 5)],
            vec![task("anchor", 7)]
        )
        .is_err()
    );
    assert!(
        PrivateSuites::freeze(
            vec![task("screen", 1)],
            vec![task("selection", 2)],
            vec![task("confirmation", 5)],
            vec![task("anchor", 7)]
        )
        .is_err()
    );
    let a = task("fingerprint", 11);
    let mut b = a.clone();
    b.prompt.push_str("changed");
    assert_ne!(
        fingerprint::suite("screen", std::slice::from_ref(&a)).unwrap(),
        fingerprint::suite("screen", &[b]).unwrap()
    );
    let mut b = a.clone();
    b.seeds = vec![13];
    assert_ne!(
        fingerprint::suite("screen", std::slice::from_ref(&a)).unwrap(),
        fingerprint::suite("screen", &[b]).unwrap()
    );
    let mut b = a.clone();
    b.checker_stamp = fingerprint::checker(&crate::HostVerifier::Golden {
        expected: b"different".to_vec(),
    })
    .unwrap();
    assert_ne!(
        fingerprint::suite("screen", &[a]).unwrap(),
        fingerprint::suite("screen", &[b]).unwrap()
    );
}
struct Fixed(Vec<u8>);
impl ModelPort for Fixed {
    fn generate(&self, _: ModelRequest) -> ModelFuture {
        let content = self.0.clone();
        Box::pin(async move {
            Ok(ModelReply {
                content,
                input_tokens: 10,
                output_tokens: 10,
            })
        })
    }
}
fn checked_suites(verifier: &crate::HostVerifier) -> PrivateSuites {
    let draw = |id: &str, seed| {
        PrivateTask::new(
            id.into(),
            "family".into(),
            "alice".into(),
            "Produce the required artifact".into(),
            verifier.clone(),
            vec![seed, seed + 1],
        )
        .unwrap()
    };
    PrivateSuites::freeze(
        vec![draw("screen", 1)],
        vec![draw("selection", 3)],
        vec![draw("confirmation", 5)],
        vec![draw("anchor", 7)],
    )
    .unwrap()
}
#[test]
fn schema_checker_runs_inside_the_real_episode_without_private_schema_in_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let verifier = crate::HostVerifier::Schema {
        schema: serde_json::json!({"type":"object","required":["ok"],"properties":{"ok":{"const":true}},"additionalProperties":false}),
    };
    let mut evaluator = CoordinatorEvaluator::new(
        checked_suites(&verifier),
        Arc::new(Fixed(br#"{"ok":true}"#.to_vec())),
        memory,
        100,
        1000,
    )
    .unwrap();
    let result = evaluator
        .evaluate(
            &state(),
            &state(),
            EvalStage::Screen,
            &Budget { max_tokens: 10_000 },
        )
        .unwrap();
    assert!(
        result
            .incumbent
            .iter()
            .all(|r| (r.quality - 1.0).abs() < f64::EPSILON)
    );
    assert!(!format!("{evaluator:?}").contains("additionalProperties"));
}
#[cfg(unix)]
#[test]
fn executable_checker_and_proof_failure_use_frozen_strong_tiers() {
    let dir = tempfile::tempdir().unwrap();
    let golden = dir.path().join("golden");
    std::fs::write(&golden, b"good").unwrap();
    let command = crate::CommandSpec {
        program: "/usr/bin/cmp".into(),
        args: vec!["{artifact}".into(), golden.to_string_lossy().into_owned()],
        workspace_root: dir.path().join("checks"),
        timeout_ms: 1000,
        max_output_bytes: 1024,
    };
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let mut evaluator = CoordinatorEvaluator::new(
        checked_suites(&crate::HostVerifier::Process(command.clone())),
        Arc::new(Fixed(b"good".to_vec())),
        memory.clone(),
        100,
        1000,
    )
    .unwrap();
    let result = evaluator
        .evaluate(
            &state(),
            &state(),
            EvalStage::Screen,
            &Budget { max_tokens: 10_000 },
        )
        .unwrap();
    assert!(
        result
            .incumbent
            .iter()
            .all(|r| (r.quality - 1.0).abs() < f64::EPSILON)
    );
    let mut lean_command = command;
    lean_command.program = "/usr/bin/true".into();
    lean_command.args = vec![];
    let verifier = crate::HostVerifier::Lean {
        command: lean_command,
        theorem: "theorem rsi_goal : True :=".into(),
    };
    let mut evaluator = CoordinatorEvaluator::new(
        checked_suites(&verifier),
        Arc::new(Fixed(b"sorry".to_vec())),
        memory,
        100,
        1000,
    )
    .unwrap();
    let result = evaluator
        .evaluate(
            &state(),
            &state(),
            EvalStage::Screen,
            &Budget { max_tokens: 10_000 },
        )
        .unwrap();
    assert!(
        result
            .incumbent
            .iter()
            .all(|r| r.quality == 0.0 && r.false_claims == 1)
    );
}
#[cfg(unix)]
#[test]
fn changing_frozen_checker_executable_is_detected_before_any_model_call() {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("checker");
    std::fs::copy("/usr/bin/cmp", &program).unwrap();
    let command = crate::CommandSpec {
        program: program.to_string_lossy().into_owned(),
        args: vec![],
        workspace_root: dir.path().join("checks"),
        timeout_ms: 1000,
        max_output_bytes: 1024,
    };
    let suites = checked_suites(&crate::HostVerifier::Process(command));
    std::fs::write(program, b"changed executable").unwrap();
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        bad: false,
    });
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let mut evaluator =
        CoordinatorEvaluator::new(suites, model.clone(), memory, 100, 1000).unwrap();
    assert!(
        evaluator
            .evaluate_h(
                &state(),
                &state(),
                EvalStage::Screen,
                &Budget { max_tokens: 10_000 }
            )
            .is_err()
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
}
#[test]
fn model_failure_or_forged_usage_is_not_fabricated_as_a_scored_run() {
    struct Broken(bool);
    impl ModelPort for Broken {
        fn generate(&self, _: ModelRequest) -> ModelFuture {
            let over = self.0;
            Box::pin(async move {
                if over {
                    Ok(ModelReply {
                        content: b"good".to_vec(),
                        input_tokens: u64::MAX,
                        output_tokens: 1,
                    })
                } else {
                    Err(Error::HostFailed)
                }
            })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    for over in [false, true] {
        let mut evaluator =
            CoordinatorEvaluator::new(suites(), Arc::new(Broken(over)), memory.clone(), 100, 1000)
                .unwrap();
        assert!(
            evaluator
                .evaluate_h(
                    &state(),
                    &state(),
                    EvalStage::Screen,
                    &Budget { max_tokens: 10_000 }
                )
                .is_err()
        );
    }
}
#[test]
fn model_binding_and_executor_boundaries_fail_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        bad: false,
    });
    assert!(CoordinatorEvaluator::new(suites(), model.clone(), memory.clone(), 0, 1000).is_err());
    let mut evaluator =
        CoordinatorEvaluator::new(suites(), model.clone(), memory.clone(), 100, 1000)
            .unwrap()
            .with_model_name("chosen".into())
            .unwrap();
    let mut candidate = state();
    candidate.package.manifest.seats[0].model = Some("chosen".into());
    assert!(
        evaluator
            .evaluate_h(
                &candidate,
                &candidate,
                EvalStage::Screen,
                &Budget { max_tokens: 10_000 }
            )
            .is_ok()
    );
    candidate.package.manifest.seats[0].model = Some("other".into());
    assert!(
        evaluator
            .evaluate_h(
                &candidate,
                &candidate,
                EvalStage::Screen,
                &Budget { max_tokens: 10_000 }
            )
            .is_err()
    );
    assert!(
        CoordinatorEvaluator::new(suites(), model.clone(), memory.clone(), 100, 1000)
            .unwrap()
            .with_model_name(String::new())
            .is_err()
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async move {
        assert!(CoordinatorEvaluator::new(suites(), model, memory, 100, 1000).is_err());
        assert!(
            evaluator
                .evaluate_anchor_once(&state(), &Budget { max_tokens: 10_000 })
                .is_err()
        );
        drop(evaluator);
    });
}

#[tokio::test]
async fn controlled_factory_requires_binding_and_retains_session_when_writer_is_missing() {
    use crate::RunnerFactory;
    use tinyhivemind_hives::{TurnDisposition, TurnRequest};
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        bad: false,
    });
    assert!(
        ControlledRunnerFactory::new(
            model.clone(),
            memory.clone(),
            task("production", 99),
            Budget { max_tokens: 0 },
            100,
            1000
        )
        .is_err()
    );
    let make = || {
        ControlledRunnerFactory::new(
            model.clone(),
            memory.clone(),
            task("production", 99),
            Budget { max_tokens: 10_000 },
            100,
            1000,
        )
        .unwrap()
    };
    assert!(make().with_model_name(String::new()).is_err());
    let factory = make().with_model_name("declared".into()).unwrap();
    let mut package = state().package;
    package.manifest.seats[0].model = Some("declared".into());
    let hive = tinyhivemind_lang::lower(&package).unwrap();
    factory.validate(&hive).unwrap();
    let runner = factory
        .build(&hive, &hive.seats[0], Some("retained"))
        .unwrap();
    for session in [Some("retained".to_owned()), None] {
        let outcome = runner
            .run(TurnRequest {
                agent_id: "alice".into(),
                session_id: session.clone(),
                messages: vec![],
                memberships: vec![],
                episode: None,
                resumption: None,
            })
            .await
            .unwrap();
        assert_eq!(outcome.disposition, TurnDisposition::Parked);
        assert_eq!(
            outcome.session_id,
            session.unwrap_or_else(|| "verified:alice".into())
        );
    }
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
    assert_eq!(factory.evidence().unwrap(), vec![]);
    assert_eq!(factory.usage().unwrap(), vec![]);
    assert!(make().validate(&hive).is_err());
}

#[tokio::test]
async fn controlled_runner_recalls_the_exact_restored_seat_memory_snapshot() {
    use tinyhivemind_hives::{Destination, HiveInfo, MemoryStorage, SendMessage};
    struct Recalled;
    impl ModelPort for Recalled {
        fn generate(&self, request: ModelRequest) -> ModelFuture {
            let recalled = request.system().contains("method: old");
            Box::pin(async move {
                Ok(ModelReply {
                    content: if recalled {
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
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
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
    let factory = Arc::new(
        ControlledRunnerFactory::new(
            Arc::new(Recalled),
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
    crate::ActivationBackend::apply(&mut host, &incumbent.package, &incumbent.memory_watermarks)
        .await
        .unwrap();
    let coordinator = host.coordinator().unwrap();
    coordinator
        .create_hive(HiveInfo {
            hive_id: "recall".into(),
            name: "Recall".into(),
            description: None,
            members: vec!["alice".into()],
        })
        .await
        .unwrap();
    coordinator
        .send_as_host(SendMessage {
            message_id: "recall".into(),
            sender: String::new(),
            destination: Destination::Hive("recall".into()),
            body: "Recall the pinned method".into(),
            thread: None,
            only_for: vec![],
            starters: vec!["alice".into()],
        })
        .await
        .unwrap();
    coordinator.run_until_idle().await.unwrap();
    assert_eq!(factory.evidence().unwrap().len(), 1);
    assert!(factory.evidence().unwrap()[0].passed);
    assert_eq!(factory.usage().unwrap()[0].input_tokens, 10);
}
