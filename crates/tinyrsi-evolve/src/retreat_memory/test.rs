//! Persistence and isolation of creative discovery history.
// Fixture failures panic only in deterministic tests.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixture results")]
use super::*;
use tinybackrooms::Memory;
#[tokio::test]
async fn reopens_discovery_history_and_isolates_namespace() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.json");
    let memory =
        DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec!["SECRET".into()])
            .unwrap();
    memory
        .store("project:rsi-h/agent:retreat", vec!["idea".into()])
        .await
        .unwrap();
    assert!(
        memory
            .store("project:rsi-other/agent:retreat", vec!["idea".into()])
            .await
            .is_err()
    );
    assert!(
        memory
            .store("project:rsi-h/agent:retreat", vec!["SECRET".into()])
            .await
            .is_err()
    );
    assert!(DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec![]).is_err());
    drop(memory);
    let memory = DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec![]).unwrap();
    assert_eq!(
        memory
            .recall("project:rsi-h/agent:retreat", "", 10)
            .await
            .unwrap(),
        vec!["idea"]
    );
    assert_eq!(
        memory
            .recall("project:rsi-h/agent:retreat", "", 0)
            .await
            .unwrap(),
        Vec::<String>::new()
    );
}
#[tokio::test]
async fn fails_closed_on_corruption_and_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m");
    std::fs::write(&path, b"broken").unwrap();
    assert!(DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec![]).is_err());
    std::fs::remove_file(&path).unwrap();
    let memory = DurableRetreatMemory::open(
        &path,
        "h",
        MemoryLimits {
            max_entries: 1,
            max_bytes: 128,
        },
        vec![],
    )
    .unwrap();
    memory
        .store("project:rsi-h/agent:retreat", vec!["first".into()])
        .await
        .unwrap();
    memory
        .store(memory.namespace(), vec!["second".into()])
        .await
        .unwrap();
    assert_eq!(
        memory.recall(memory.namespace(), "", 10).await.unwrap(),
        vec!["second"]
    );
    assert!(
        memory
            .store(memory.namespace(), vec!["x".repeat(129)])
            .await
            .is_err()
    );
    memory
        .record_outcome(RetreatMemoryOutcome {
            epoch: 1,
            candidate_id: "id".into(),
            status: RetreatOutcomeStatus::Rejected,
        })
        .unwrap();
    assert_eq!(memory.outcomes().unwrap().len(), 1);
}
#[test]
fn rejects_invalid_configuration_wrong_hive_and_contaminated_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m");
    for hive in ["", "../bad"] {
        assert!(DurableRetreatMemory::open(&path, hive, MemoryLimits::default(), vec![]).is_err());
    }
    assert!(
        DurableRetreatMemory::open(
            &path,
            "h",
            MemoryLimits {
                max_entries: 0,
                max_bytes: 1
            },
            vec![]
        )
        .is_err()
    );
    assert!(
        DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec![String::new()])
            .is_err()
    );
    let m = DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec!["secret".into()])
        .unwrap();
    assert!(
        m.record_outcome(RetreatMemoryOutcome {
            epoch: 1,
            candidate_id: "secret".into(),
            status: RetreatOutcomeStatus::Approved
        })
        .is_err()
    );
    m.record_outcome(RetreatMemoryOutcome {
        epoch: 1,
        candidate_id: "id".into(),
        status: RetreatOutcomeStatus::Approved,
    })
    .unwrap();
    drop(m);
    assert!(DurableRetreatMemory::open(&path, "other", MemoryLimits::default(), vec![]).is_err());
    assert_eq!(
        DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec![])
            .unwrap()
            .outcomes()
            .unwrap()[0]
            .status,
        RetreatOutcomeStatus::Approved
    );
}
#[tokio::test]
async fn protected_history_and_size_are_revalidated_on_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m");
    let m = DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec![]).unwrap();
    m.store("project:rsi-h/agent:retreat", vec!["later-secret".into()])
        .await
        .unwrap();
    drop(m);
    assert!(
        DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec!["secret".into()])
            .is_err()
    );
    assert!(
        DurableRetreatMemory::open(
            &path,
            "h",
            MemoryLimits {
                max_entries: 1,
                max_bytes: 1
            },
            vec![]
        )
        .is_err()
    );
}
#[tokio::test]
async fn actual_engine_reopens_and_recalls_prior_discoveries() {
    use std::sync::Arc;
    use tinybackrooms::{
        Constraints, Context, Engine, GenerateRequest, OutputSpec, RunConfig, ScorerKind,
        ScorerSpec, ScriptedConversant,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m");
    let request = GenerateRequest {
        input: Context {
            brief: "name".into(),
            seeds: vec![],
            constraints: Constraints::default(),
            memory_namespace: Some("project:rsi-h/agent:retreat".into()),
        },
        output: OutputSpec {
            item_name: "name".into(),
            item_guidance: None,
            schema: serde_json::json!({"type":"object","required":["name"],"properties":{"name":{"type":"string"}}}),
        },
        config: RunConfig {
            variations: 1,
            generations: 2,
            items_per_variation: 1,
            ..RunConfig::default()
        },
    };
    let mut request = request;
    request.config.scoring.scorers = vec![ScorerSpec {
        kind: ScorerKind::Originality,
        weight: 1.0,
    }];
    for iteration in 0..2 {
        let memory = Arc::new(
            DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec![]).unwrap(),
        );
        let engine = Engine::new(Arc::new(ScriptedConversant::new(
            |_| Ok("name".into()),
            |_| Ok(serde_json::json!({"items":[{"name":"same discovery"}]})),
        )))
        .with_memory(memory.clone());
        let result = engine.generate(&request).await.unwrap();
        assert_eq!(result.warnings, Vec::<String>::new());
        if iteration == 1 {
            assert!(result.items[0].scores["originality"] < 1e-9);
        }
        assert_eq!(
            memory
                .recall("project:rsi-h/agent:retreat", "", 10)
                .await
                .unwrap()
                .len(),
            iteration + 1
        );
        assert_eq!(
            memory.outcomes().unwrap(),
            Vec::<RetreatMemoryOutcome>::new()
        );
    }
}
#[tokio::test]
async fn rejects_json_escaped_protected_values_before_durable_storage() {
    let dir = tempfile::tempdir().unwrap();
    let memory = DurableRetreatMemory::open(
        dir.path().join("m"),
        "h",
        MemoryLimits::default(),
        vec!["secret\nline".into()],
    )
    .unwrap();
    let encoded = serde_json::json!({"mechanism":"secret\nline"}).to_string();
    assert!(memory.validate_public_text(&encoded).is_err());
    for value in [
        serde_json::json!(["secret\nline"]),
        serde_json::json!({"secret\nline":false}),
        serde_json::json!("secret\nline"),
    ] {
        assert!(memory.validate_public_text(&value.to_string()).is_err());
    }
    assert!(memory.validate_public_text("[1,true,null]").is_ok());
    assert!(
        memory
            .store(memory.namespace(), vec![encoded])
            .await
            .is_err()
    );
    assert!(
        memory
            .record_outcome(RetreatMemoryOutcome {
                epoch: 1,
                candidate_id: "ok".into(),
                status: RetreatOutcomeStatus::Rejected
            })
            .is_ok()
    );
    assert!(
        memory
            .record_outcome(RetreatMemoryOutcome {
                epoch: 1,
                candidate_id: "ok".into(),
                status: RetreatOutcomeStatus::Rejected
            })
            .is_ok()
    );
    assert!(
        memory
            .record_outcome(RetreatMemoryOutcome {
                epoch: 1,
                candidate_id: "ok".into(),
                status: RetreatOutcomeStatus::Approved
            })
            .is_err()
    );
}
#[cfg(unix)]
#[test]
fn refuses_memory_and_lock_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m");
    for target in [&path, &path.with_extension("lock")] {
        std::os::unix::fs::symlink(dir.path().join("missing"), target).unwrap();
        assert!(matches!(
            DurableRetreatMemory::open(&path, "h", MemoryLimits::default(), vec![]),
            Err(Error::ArchiveIo)
        ));
        std::fs::remove_file(target).unwrap();
    }
}

#[tokio::test]
async fn rolling_windows_keep_latest_epochs_and_outcomes_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rolling");
    let limits = MemoryLimits {
        max_entries: 2,
        max_bytes: 256,
    };
    let memory = DurableRetreatMemory::open(&path, "h", limits.clone(), vec![]).unwrap();
    for epoch in 1..=5 {
        memory
            .store(memory.namespace(), vec![format!("idea-{epoch}")])
            .await
            .unwrap();
        memory
            .record_outcome(RetreatMemoryOutcome {
                epoch,
                candidate_id: format!("id-{epoch}"),
                status: RetreatOutcomeStatus::Rejected,
            })
            .unwrap();
    }
    drop(memory);
    let memory = DurableRetreatMemory::open(&path, "h", limits, vec![]).unwrap();
    assert_eq!(
        memory.recall(memory.namespace(), "", 10).await.unwrap(),
        vec!["idea-4", "idea-5"]
    );
    assert_eq!(
        memory
            .outcomes()
            .unwrap()
            .iter()
            .map(|o| o.epoch)
            .collect::<Vec<_>>(),
        vec![4, 5]
    );
}

#[tokio::test]
async fn byte_eviction_is_independent_and_rejects_every_incoming_protected_or_oversized_item() {
    let dir = tempfile::tempdir().unwrap();
    let memory = DurableRetreatMemory::open(
        dir.path().join("rolling"),
        "h",
        MemoryLimits {
            max_entries: 2,
            max_bytes: 90,
        },
        vec!["SECRET".into()],
    )
    .unwrap();
    memory
        .store(memory.namespace(), vec!["a".repeat(70)])
        .await
        .unwrap();
    memory
        .store(memory.namespace(), vec!["b".repeat(70)])
        .await
        .unwrap();
    assert_eq!(
        memory.recall(memory.namespace(), "", 10).await.unwrap(),
        vec!["b".repeat(70)]
    );
    for first in ["SECRET".into(), "x".repeat(129)] {
        assert!(
            memory
                .store(memory.namespace(), vec![first, "one".into(), "two".into()])
                .await
                .is_err()
        );
        assert_eq!(
            memory.recall(memory.namespace(), "", 10).await.unwrap(),
            vec!["b".repeat(70)]
        );
    }
    for epoch in 1..=4 {
        memory
            .record_outcome(RetreatMemoryOutcome {
                epoch,
                candidate_id: format!("id-{epoch}"),
                status: RetreatOutcomeStatus::Approved,
            })
            .unwrap();
    }
    assert_eq!(memory.outcomes().unwrap().last().unwrap().epoch, 4);
    assert!(memory.outcomes().unwrap().len() < 2);
    assert_eq!(
        memory.recall(memory.namespace(), "", 10).await.unwrap(),
        vec!["b".repeat(70)]
    );
    for candidate_id in ["SECRET".into(), "x".repeat(129), String::new()] {
        assert!(
            memory
                .record_outcome(RetreatMemoryOutcome {
                    epoch: 5,
                    candidate_id,
                    status: RetreatOutcomeStatus::Approved
                })
                .is_err()
        );
    }
    assert_eq!(memory.outcomes().unwrap().last().unwrap().epoch, 4);
}

#[test]
fn outcome_idempotency_is_scoped_to_the_retained_window() {
    let dir = tempfile::tempdir().unwrap();
    let memory = DurableRetreatMemory::open(
        dir.path().join("rolling"),
        "h",
        MemoryLimits {
            max_entries: 1,
            max_bytes: 256,
        },
        vec![],
    )
    .unwrap();
    let first = RetreatMemoryOutcome {
        epoch: 1,
        candidate_id: "one".into(),
        status: RetreatOutcomeStatus::Rejected,
    };
    memory.record_outcome(first.clone()).unwrap();
    memory.record_outcome(first.clone()).unwrap();
    let mut changed = first;
    changed.status = RetreatOutcomeStatus::Approved;
    assert!(memory.record_outcome(changed.clone()).is_err());
    memory
        .record_outcome(RetreatMemoryOutcome {
            epoch: 2,
            candidate_id: "two".into(),
            status: RetreatOutcomeStatus::Rejected,
        })
        .unwrap();
    memory.record_outcome(changed.clone()).unwrap();
    assert_eq!(memory.outcomes().unwrap(), vec![changed]);
}
