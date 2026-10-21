//! Host verification rejects forged identity and invalid artifacts.
// Fixture failures panic only in deterministic tests.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixture results")]
use super::*;
fn binding() -> VerificationBinding {
    VerificationBinding {
        episode_id: "ep".into(),
        seat_id: "seat".into(),
        node_id: "node".into(),
        artifact_id: "a".into(),
        transcript_refs: vec![1],
    }
}
fn registry(verifier: HostVerifier) -> VerifierRegistry {
    VerifierRegistry::new(vec![VerifierDeclaration {
        node_id: "node".into(),
        owner_seat: "seat".into(),
        verifier,
    }])
    .unwrap()
}
#[test]
fn golden_is_exact_and_forged_bindings_are_rejected() {
    let r = registry(HostVerifier::Golden {
        expected: b"yes".to_vec(),
    });
    let b = binding();
    let mut a = ArtifactSubmission {
        binding: b.clone(),
        content: b"yes".to_vec(),
    };
    assert!(r.verify(&b, &a).unwrap().passed);
    a.content = b"no".to_vec();
    assert!(!r.verify(&b, &a).unwrap().passed);
    a.binding.episode_id = "other".into();
    assert!(r.verify(&b, &a).is_err());
}
#[test]
fn evidence_binds_the_frozen_checker_identity_and_observed_exit_status() {
    let command = CommandSpec {
        program: "/usr/bin/python3".into(),
        args: vec!["-c".into(), "import sys; sys.exit(0)".into()],
        workspace_root: std::path::PathBuf::from("target/rsi-workspaces"),
        timeout_ms: 1000,
        max_output_bytes: 1024,
    };
    let registry = registry(HostVerifier::Process(command.clone()));
    let binding = binding();
    let report = registry
        .verify(
            &binding,
            &ArtifactSubmission {
                binding: binding.clone(),
                content: b"candidate".to_vec(),
            },
        )
        .unwrap();
    assert_eq!(
        report.checker_identity,
        Some(checker_identity(&HostVerifier::Process(command.clone())).unwrap())
    );
    assert_eq!(report.exit_code, Some(0));
    let mut changed = command;
    changed.args.push("changed".into());
    let changed_identity = checker_identity(&HostVerifier::Process(changed)).unwrap();
    assert_ne!(
        report.checker_identity.as_deref(),
        Some(changed_identity.as_str())
    );
}

#[test]
fn verification_evidence_serialization_preserves_checker_identity_and_legacy_records() {
    let evidence = VerificationEvidence {
        binding: binding(),
        artifact_digest: "a".repeat(64),
        checker_identity: Some(format!("sha256:{}", "b".repeat(64))),
        passed: true,
        quality: 1.0,
        exit_code: Some(0),
    };
    let value = serde_json::to_value(&evidence).unwrap();
    assert_eq!(
        value["checker_identity"],
        format!("sha256:{}", "b".repeat(64))
    );
    assert_eq!(
        serde_json::from_value::<VerificationEvidence>(value.clone()).unwrap(),
        evidence
    );

    let mut legacy = value;
    legacy.as_object_mut().unwrap().remove("checker_identity");
    assert_eq!(
        serde_json::from_value::<VerificationEvidence>(legacy)
            .unwrap()
            .checker_identity,
        None
    );
}

#[test]
fn checker_identity_resolves_path_executables_and_rejects_missing_ones() {
    let command = CommandSpec {
        program: "python3".into(),
        args: vec![],
        workspace_root: std::path::PathBuf::from("target/rsi-workspaces"),
        timeout_ms: 1000,
        max_output_bytes: 1024,
    };
    assert!(checker_identity(&HostVerifier::Process(command.clone())).is_ok());

    let mut missing = command;
    missing.program = "tinyrsi-no-such-checker".into();
    assert!(checker_identity(&HostVerifier::Process(missing)).is_err());
}
#[test]
#[cfg(unix)]
fn verifier_registry_rejects_an_executable_changed_after_freezing() {
    let directory = tempfile::tempdir().unwrap();
    let program = directory.path().join("checker");
    std::fs::write(&program, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let registry = registry(HostVerifier::Process(CommandSpec {
        program: program.to_string_lossy().into_owned(),
        args: vec![],
        workspace_root: std::path::PathBuf::from("target/rsi-workspaces"),
        timeout_ms: 1000,
        max_output_bytes: 1024,
    }));
    std::fs::write(&program, "#!/bin/sh\nexit 1\n").unwrap();
    let binding = binding();
    assert!(
        registry
            .verify(
                &binding,
                &ArtifactSubmission {
                    binding: binding.clone(),
                    content: b"candidate".to_vec(),
                },
            )
            .is_err()
    );
}
#[test]
fn validates_json_schema_and_rejects_invalid_registry() {
    let r = registry(HostVerifier::Schema {
        schema: serde_json::json!({"type":"integer"}),
    });
    let b = binding();
    assert!(
        r.verify(
            &b,
            &ArtifactSubmission {
                binding: b.clone(),
                content: b"3".to_vec()
            }
        )
        .unwrap()
        .passed
    );
    assert!(
        !r.verify(
            &b,
            &ArtifactSubmission {
                binding: b.clone(),
                content: b"bad".to_vec()
            }
        )
        .unwrap()
        .passed
    );
    assert!(VerifierRegistry::new(vec![]).is_err());
}
#[test]
fn direct_process_exit_timeout_and_output_bounds() {
    let command = CommandSpec {
        program: "/usr/bin/python3".into(),
        args: vec![
            "-c".into(),
            "import pathlib,sys;sys.exit(0 if pathlib.Path(sys.argv[1]).read_text()=='yes' else 1)"
                .into(),
            "{artifact}".into(),
        ],
        workspace_root: std::path::PathBuf::from("target/rsi-workspaces"),
        timeout_ms: 1000,
        max_output_bytes: 1024,
    };
    let r = registry(HostVerifier::Process(command.clone()));
    let b = binding();
    assert!(
        r.verify(
            &b,
            &ArtifactSubmission {
                binding: b.clone(),
                content: b"yes".to_vec()
            }
        )
        .unwrap()
        .passed
    );
    assert!(
        !r.verify(
            &b,
            &ArtifactSubmission {
                binding: b.clone(),
                content: b"no".to_vec()
            }
        )
        .unwrap()
        .passed
    );
    let mut timed = command.clone();
    timed.args = vec!["-c".into(), "import time;time.sleep(1)".into()];
    timed.timeout_ms = 10;
    assert!(
        !registry(HostVerifier::Process(timed))
            .verify(
                &b,
                &ArtifactSubmission {
                    binding: b.clone(),
                    content: vec![]
                }
            )
            .unwrap()
            .passed
    );
    let mut flood = command;
    flood.args = vec!["-c".into(), "print('x'*5000)".into()];
    flood.max_output_bytes = 10;
    assert!(
        !registry(HostVerifier::Process(flood))
            .verify(
                &b,
                &ArtifactSubmission {
                    binding: b.clone(),
                    content: vec![]
                }
            )
            .unwrap()
            .passed
    );
}
#[test]
fn rejects_each_forged_identity_and_invalid_declarations() {
    let r = registry(HostVerifier::Golden { expected: vec![] });
    let b = binding();
    for field in 0..5 {
        let mut forged = b.clone();
        match field {
            0 => forged.episode_id = "other".into(),
            1 => forged.seat_id = "other".into(),
            2 => forged.node_id = "other".into(),
            3 => forged.artifact_id = "other".into(),
            _ => forged.transcript_refs = vec![99],
        }
        assert!(
            r.verify(
                &b,
                &ArtifactSubmission {
                    binding: forged,
                    content: vec![]
                }
            )
            .is_err()
        );
    }
    for refs in [vec![], vec![1, 1], vec![2, 1]] {
        let mut bad = b.clone();
        bad.transcript_refs = refs;
        assert!(
            r.verify(
                &bad,
                &ArtifactSubmission {
                    binding: bad.clone(),
                    content: vec![]
                }
            )
            .is_err()
        );
    }
    let declaration = VerifierDeclaration {
        node_id: "n".into(),
        owner_seat: "s".into(),
        verifier: HostVerifier::Golden { expected: vec![] },
    };
    assert!(VerifierRegistry::new(vec![declaration.clone(), declaration]).is_err());
    assert!(
        VerifierRegistry::new(vec![VerifierDeclaration {
            node_id: String::new(),
            owner_seat: "s".into(),
            verifier: HostVerifier::Schema {
                schema: serde_json::json!({"type":"nonsense"})
            }
        }])
        .is_err()
    );
}
struct Producer {
    content: Vec<u8>,
    forge: bool,
}
impl ArtifactProducer for Producer {
    fn produce(
        &self,
        _request: tinyhivemind_hives::TurnRequest,
        mut binding: VerificationBinding,
    ) -> ArtifactFuture {
        let content = self.content.clone();
        if self.forge {
            binding.transcript_refs = vec![999];
        }
        Box::pin(async move { Ok(ArtifactSubmission { binding, content }) })
    }
}
struct RejectEvidence;
impl VerificationObserver for RejectEvidence {
    fn observe(&self, _: &VerificationEvidence) -> crate::Result<()> {
        Err(crate::Error::HostFailed)
    }
}
#[tokio::test]
async fn actual_coordinator_parks_failed_forged_or_unpersisted_and_completes_verified() {
    use std::sync::Arc;
    use tinyhivemind_hives::{
        AgentRegistration, Coordinator, CoordinatorOptions, Destination, EpisodePhase, HiveInfo,
        MemoryStorage, SendMessage,
    };
    for (content, forge, observer_fail, passed) in [
        (b"yes".to_vec(), false, false, true),
        (b"no".to_vec(), false, false, false),
        (b"yes".to_vec(), true, false, false),
        (b"yes".to_vec(), false, true, false),
    ] {
        let c = Arc::new(
            Coordinator::new(
                "runtime".into(),
                Arc::new(MemoryStorage::new()),
                CoordinatorOptions::default(),
            )
            .await
            .unwrap(),
        );
        let blueprint = crate::Blueprint::new(vec![crate::BlueprintNode {
            id: "node".into(),
            statement: "exact answer".into(),
            owner_role: "seat".into(),
            dependencies: vec![],
            verifier: crate::Verifier {
                kind: crate::VerifierKind::Execution,
                spec: "private".into(),
            },
            status: crate::NodeStatus::Open,
        }])
        .unwrap();
        let mut runner = VerifiedRunner::new(
            &c,
            Arc::new(Producer { content, forge }),
            Arc::new(registry(HostVerifier::Golden {
                expected: b"yes".to_vec(),
            })),
            blueprint,
        )
        .unwrap();
        if observer_fail {
            runner = runner.with_observer(Arc::new(RejectEvidence));
        }
        let runner = Arc::new(runner);
        c.register_agent(AgentRegistration {
            agent_id: "seat".into(),
            runtime_id: "runtime".into(),
            runner: runner.clone(),
        })
        .await
        .unwrap();
        c.create_hive(HiveInfo {
            hive_id: "h".into(),
            name: "h".into(),
            description: None,
            members: vec!["seat".into()],
        })
        .await
        .unwrap();
        c.send_as_host(SendMessage {
            message_id: "m".into(),
            sender: "ignored".into(),
            destination: Destination::Hive("h".into()),
            body: "task".into(),
            thread: None,
            only_for: vec![],
            starters: vec![],
        })
        .await
        .unwrap();
        c.run_until_idle().await.unwrap();
        let phase = &c.episodes().unwrap()[0].phase;
        assert_eq!(
            *phase,
            if passed {
                EpisodePhase::Settled
            } else {
                EpisodePhase::AwaitingRelease
            }
        );
        assert_eq!(runner.evidence().unwrap().len(), usize::from(!forge));
    }
}
#[test]
fn lean_rejects_generated_axioms_sorry_and_declarations_before_execution() {
    let command = CommandSpec {
        program: "python3".into(),
        args: vec!["{artifact}".into()],
        workspace_root: "target/rsi-workspaces".into(),
        timeout_ms: 10,
        max_output_bytes: 100,
    };
    let r = registry(HostVerifier::Lean {
        command,
        theorem: "theorem rsi_goal : True :=".into(),
    });
    let b = binding();
    for proof in [
        "by sorry",
        "by admit",
        "by unsafe",
        "by exact True.intro\naxiom evil : False",
        "by run_tac pure ()",
    ] {
        assert!(
            !r.verify(
                &b,
                &ArtifactSubmission {
                    binding: b.clone(),
                    content: proof.as_bytes().to_vec()
                }
            )
            .unwrap()
            .passed
        );
    }
}
#[test]
fn proof_kernel_report_must_exclude_admission_or_unknown_axioms() {
    let dir = std::path::PathBuf::from("target/rsi-workspaces");
    let b = binding();
    for (report, expected) in [
        ("'rsi_goal' depends on axioms: []", true),
        ("'rsi_goal' depends on axioms: [propext]", true),
        ("'rsi_goal' depends on axioms: [sorryAx]", false),
        ("checked", false),
    ] {
        let command = CommandSpec {
            program: "/usr/bin/python3".into(),
            args: vec!["-c".into(), format!("print({report:?})")],
            workspace_root: dir.clone(),
            timeout_ms: 1000,
            max_output_bytes: 1024,
        };
        let r = registry(HostVerifier::Lean {
            command,
            theorem: "theorem rsi_goal : True :=".into(),
        });
        assert_eq!(
            r.verify(
                &b,
                &ArtifactSubmission {
                    binding: b.clone(),
                    content: b"by exact True.intro".to_vec()
                }
            )
            .unwrap()
            .passed,
            expected
        );
    }
}
/// Optional real Lean smoke; deterministic process mocks above do not prove Lean integration.
#[test]
fn live_stock_lean_checks_frozen_theorem_and_kernel_axioms() {
    let Ok(program) = std::env::var("TINYRSI_LEAN") else {
        return;
    };
    let command = CommandSpec {
        program,
        args: vec!["{artifact}".into()],
        workspace_root: "target/rsi-workspaces".into(),
        timeout_ms: 10000,
        max_output_bytes: 65536,
    };
    let r = registry(HostVerifier::Lean {
        command,
        theorem: "theorem rsi_goal : True :=".into(),
    });
    let b = binding();
    let evidence = r
        .verify(
            &b,
            &ArtifactSubmission {
                binding: b.clone(),
                content: b"by exact True.intro".to_vec(),
            },
        )
        .unwrap();
    assert!(evidence.passed);
}
#[test]
fn rejects_schema_configuration_and_observes_process_host_failures() {
    assert!(
        VerifierRegistry::new(vec![VerifierDeclaration {
            node_id: "node".into(),
            owner_seat: "seat".into(),
            verifier: HostVerifier::Schema {
                schema: serde_json::json!({"type":"bad"})
            }
        }])
        .is_err()
    );
    let b = binding();
    let missing = CommandSpec {
        program: "/definitely/missing/checker".into(),
        args: vec![],
        workspace_root: "target/rsi-workspaces".into(),
        timeout_ms: 100,
        max_output_bytes: 100,
    };
    assert!(
        VerifierRegistry::new(vec![VerifierDeclaration {
            node_id: "node".into(),
            owner_seat: "seat".into(),
            verifier: HostVerifier::Process(missing.clone()),
        }])
        .is_err()
    );
    let mut invalid = missing.clone();
    invalid.timeout_ms = 0;
    assert!(
        VerifierRegistry::new(vec![VerifierDeclaration {
            node_id: "n".into(),
            owner_seat: "seat".into(),
            verifier: HostVerifier::Process(invalid)
        }])
        .is_err()
    );
    let mut invalid = missing.clone();
    invalid.max_output_bytes = 0;
    assert!(
        VerifierRegistry::new(vec![VerifierDeclaration {
            node_id: "n".into(),
            owner_seat: "seat".into(),
            verifier: HostVerifier::Process(invalid)
        }])
        .is_err()
    );
    let mut invalid = missing.clone();
    invalid.program = String::new();
    assert!(
        VerifierRegistry::new(vec![VerifierDeclaration {
            node_id: "n".into(),
            owner_seat: "seat".into(),
            verifier: HostVerifier::Process(invalid)
        }])
        .is_err()
    );
    let mut invalid = missing.clone();
    invalid.program = "python3".into();
    invalid.workspace_root = "/dev/null/workspace".into();
    assert!(
        registry(HostVerifier::Process(invalid))
            .verify(
                &b,
                &ArtifactSubmission {
                    binding: b.clone(),
                    content: vec![]
                }
            )
            .is_err()
    );
    let mut invalid = b.clone();
    invalid.seat_id = "forged".into();
    assert!(
        registry(HostVerifier::Golden { expected: vec![] })
            .verify(
                &invalid,
                &ArtifactSubmission {
                    binding: invalid.clone(),
                    content: vec![]
                }
            )
            .is_err()
    );
    let mut invalid = b.clone();
    invalid.node_id = "missing".into();
    assert!(
        registry(HostVerifier::Golden { expected: vec![] })
            .verify(
                &invalid,
                &ArtifactSubmission {
                    binding: invalid.clone(),
                    content: vec![]
                }
            )
            .is_err()
    );
}

mod lake;
mod proof;
mod runner;
