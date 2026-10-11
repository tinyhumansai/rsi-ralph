//! Safe receipt persistence, failure isolation and formatter occurrence regressions.
use super::*;
use crate::{
    GenerationAudit, GenerationCallStatus as Status, GenerationDisposition as Disposition,
};
fn store(journal: &EpochJournal) -> super::super::receipts::ReceiptStore {
    super::super::receipts::ReceiptStore::create(
        &journal.path,
        1,
        "a".repeat(64),
        GenerationMode::Direct,
        journal.writer_lock.clone(),
    )
    .unwrap()
}
#[test]
fn pending_calls_survive_and_keep_the_writer_lock_until_callbacks_drop() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("j");
    let journal = EpochJournal::open(&path).unwrap();
    let receipts = store(&journal);
    let first = receipts.begin().unwrap();
    let second = receipts.begin().unwrap();
    receipts
        .finish(
            second,
            Status::Returned,
            vec![Disposition::Decoded, Disposition::Decoded],
        )
        .unwrap();
    assert!(receipts.settled().is_err());
    let audit = super::super::receipts::read(&path, 1, &"a".repeat(64))
        .unwrap()
        .unwrap();
    assert_eq!(audit.calls[0].status, Status::Pending);
    assert_eq!(audit.calls[1].id, 2);
    drop(journal);
    assert!(matches!(
        EpochJournal::open(&path),
        Err(Error::ArchiveLocked)
    ));
    receipts.finish(first, Status::Failed, vec![]).unwrap();
    receipts.settled().unwrap();
    assert!(receipts.finish(first, Status::Failed, vec![]).is_err());
    assert!(receipts.finish(99, Status::Failed, vec![]).is_err());
    let third = receipts.begin().unwrap();
    assert!(receipts.finish(third, Status::Pending, vec![]).is_err());
    assert!(
        receipts
            .finish(third, Status::Failed, vec![Disposition::Decoded])
            .is_err()
    );
    receipts.finish(third, Status::Malformed, vec![]).unwrap();
    drop(receipts);
    EpochJournal::open(path).unwrap();
}
#[test]
fn corrupt_sidecars_and_failed_writes_stop_further_generation() {
    let dir = tempfile::tempdir().unwrap();
    let journal = EpochJournal::open(dir.path().join("j")).unwrap();
    assert!(journal.generation_receipts(999).unwrap().is_none());
    let receipts = store(&journal);
    let path = super::super::receipts::path(&journal.path, 1);
    assert!(
        super::super::receipts::ReceiptStore::create(
            &journal.path,
            1,
            "a".repeat(64),
            GenerationMode::Direct,
            journal.writer_lock.clone()
        )
        .is_err()
    );
    let original = std::fs::read(&path).unwrap();
    let audit: GenerationAudit = serde_json::from_slice(&original).unwrap();
    for field in ["version", "epoch", "fingerprint", "calls"] {
        let mut changed = serde_json::to_value(&audit).unwrap();
        changed[field] = match field {
            "fingerprint" => serde_json::json!("b".repeat(64)),
            "calls" => serde_json::json!([{"id":2,"status":"failed","items":["decoded"]}]),
            _ => serde_json::json!(9),
        };
        std::fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(matches!(
            super::super::receipts::read(&journal.path, 1, &"a".repeat(64)),
            Err(Error::ArchiveCorrupt)
        ));
    }
    std::fs::write(&path, "broken").unwrap();
    assert!(super::super::receipts::read(&journal.path, 1, &"a".repeat(64)).is_err());
    std::fs::File::create(&path)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert!(matches!(
        super::super::receipts::read(&journal.path, 1, &"a".repeat(64)),
        Err(Error::ArchiveCorrupt)
    ));
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(receipts.begin().is_err());
    std::fs::remove_dir(&path).unwrap();
    assert!(receipts.begin().is_err());
    assert!(receipts.settled().is_err());
    assert!(
        super::super::receipts::read(&journal.path, 1, &"a".repeat(64))
            .unwrap()
            .is_none()
    );
}
struct Payload(serde_json::Value);
impl ModelPort for Payload {
    fn generate(&self, _: crate::ModelRequest) -> crate::ModelFuture {
        let content = serde_json::to_vec(&self.0).unwrap();
        Box::pin(async move {
            Ok(crate::ModelReply {
                content,
                input_tokens: 10,
                output_tokens: 10,
            })
        })
    }
}
#[tokio::test]
async fn duplicate_private_and_malformed_occurrences_leave_only_safe_receipts() {
    for (value, expected, status) in [
        (
            serde_json::json!({"items":[proposal(),proposal()]}),
            vec![Disposition::Decoded, Disposition::Decoded],
            Status::Returned,
        ),
        (
            {
                let mut p = proposal();
                p.risk = "private canary".into();
                serde_json::json!({"items":[p]})
            },
            vec![Disposition::PrivacyRejected],
            Status::Returned,
        ),
        (
            serde_json::json!({"items":[{"private":"private canary"}]}),
            vec![Disposition::SchemaRejected],
            Status::Returned,
        ),
        (
            serde_json::json!({"private":"private canary"}),
            vec![],
            Status::Malformed,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
        let memory = Arc::new(
            DurableRetreatMemory::open(
                dir.path().join("m"),
                "h",
                crate::MemoryLimits::default(),
                vec!["private canary".into()],
            )
            .unwrap(),
        );
        let path = dir.path().join("j");
        let mut journal = EpochJournal::open(&path).unwrap();
        let cap = budget();
        let _ = prepare_retreat_epoch(
            &mut journal,
            Arc::new(Payload(value)),
            memory,
            inputs(
                &archive,
                &checkpoint,
                &dossier,
                &epoch,
                GenerationMode::Direct,
                &cap,
            ),
        )
        .await;
        let audit = journal.generation_receipts(1).unwrap().unwrap();
        assert_eq!(audit.calls[0].status, status);
        assert_eq!(audit.calls[0].items, expected);
        let bytes = std::fs::read(super::super::receipts::path(&path, 1)).unwrap();
        assert!(!String::from_utf8(bytes).unwrap().contains("private canary"));
        drop(journal);
        let journal = EpochJournal::open(path).unwrap();
        assert_eq!(journal.generation_receipts(1).unwrap(), Some(audit));
    }
}
#[cfg(unix)]
#[test]
fn symlink_sidecars_are_rejected_without_overwriting_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let journal = EpochJournal::open(dir.path().join("j")).unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, b"private").unwrap();
    let path = super::super::receipts::path(&journal.path, 1);
    std::os::unix::fs::symlink(&target, &path).unwrap();
    assert!(super::super::receipts::read(&journal.path, 1, &"a".repeat(64)).is_err());
    assert!(
        super::super::receipts::ReceiptStore::create(
            &journal.path,
            1,
            "a".repeat(64),
            GenerationMode::Direct,
            journal.writer_lock.clone()
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"private");
}
#[tokio::test]
async fn real_engine_repairs_keep_distinct_failed_and_successful_call_ids() {
    struct Repair(AtomicUsize);
    impl ModelPort for Repair {
        fn generate(&self, request: crate::ModelRequest) -> crate::ModelFuture {
            if request.system().starts_with("You read a conversation") {
                let value = if self.0.fetch_add(1, Ordering::Relaxed) == 0 {
                    serde_json::json!({"items":[{}]})
                } else {
                    serde_json::json!({"items":[proposal()]})
                };
                Payload(value).generate(request)
            } else {
                Script {
                    calls: Arc::new(AtomicUsize::new(0)),
                    fail: false,
                    invalid: false,
                }
                .generate(request)
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let memory = Arc::new(
        DurableRetreatMemory::open(
            dir.path().join("m"),
            "h",
            crate::MemoryLimits::default(),
            vec![],
        )
        .unwrap(),
    );
    let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
    let mut cap = budget();
    cap.variations = 1;
    cap.items_per_variation = 1;
    cap.select_top_k = 1;
    prepare_retreat_epoch(
        &mut journal,
        Arc::new(Repair(AtomicUsize::new(0))),
        memory,
        inputs(
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            GenerationMode::RetreatWithoutMemory,
            &cap,
        ),
    )
    .await
    .unwrap();
    let audit = journal.generation_receipts(1).unwrap().unwrap();
    assert_eq!(audit.mode, GenerationMode::RetreatWithoutMemory);
    assert_eq!(audit.calls.len(), 2);
    assert_eq!(audit.calls[0].items, vec![Disposition::SchemaRejected]);
    assert_eq!(audit.calls[1].items, vec![Disposition::Decoded]);
    assert_eq!(audit.calls[1].id, 2);
}
#[tokio::test]
async fn completed_journals_reject_missing_pending_or_mismatched_receipts_and_read_legacy() {
    let dir = tempfile::tempdir().unwrap();
    let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let memory = Arc::new(
        DurableRetreatMemory::open(
            dir.path().join("m"),
            "h",
            crate::MemoryLimits::default(),
            vec![],
        )
        .unwrap(),
    );
    let path = dir.path().join("j");
    let mut journal = EpochJournal::open(&path).unwrap();
    let cap = budget();
    prepare_retreat_epoch(
        &mut journal,
        Arc::new(Payload(serde_json::json!({"items":[proposal()]}))),
        memory,
        inputs(
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            GenerationMode::Direct,
            &cap,
        ),
    )
    .await
    .unwrap();
    let receipt_path = super::super::receipts::path(&path, 1);
    let original = std::fs::read(&receipt_path).unwrap();
    drop(journal);
    for changed in [
        {
            let mut audit: GenerationAudit = serde_json::from_slice(&original).unwrap();
            audit.mode = GenerationMode::Retreat;
            audit
        },
        {
            let mut audit: GenerationAudit = serde_json::from_slice(&original).unwrap();
            audit.calls[0].status = Status::Pending;
            audit.calls[0].items.clear();
            audit
        },
    ] {
        std::fs::write(&receipt_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(matches!(
            EpochJournal::open(&path),
            Err(Error::ArchiveCorrupt)
        ));
    }
    std::fs::remove_file(&receipt_path).unwrap();
    assert!(matches!(
        EpochJournal::open(&path),
        Err(Error::ArchiveCorrupt)
    ));
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    legacy["entries"]["1"]
        .as_object_mut()
        .unwrap()
        .remove("generation_receipts");
    legacy["entries"]["1"]
        .as_object_mut()
        .unwrap()
        .remove("generation_mode");
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let journal = EpochJournal::open(path).unwrap();
    assert!(journal.generation_receipts(1).unwrap().is_none());
}
#[tokio::test]
async fn receipt_storage_failures_prevent_dispatch_and_successful_preparation() {
    struct Sabotage(std::path::PathBuf);
    impl ModelPort for Sabotage {
        fn generate(&self, request: crate::ModelRequest) -> crate::ModelFuture {
            std::fs::rename(&self.0, self.0.with_extension("saved")).unwrap();
            std::fs::create_dir(&self.0).unwrap();
            Payload(serde_json::json!({"items":[proposal()]})).generate(request)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let memory = Arc::new(
        DurableRetreatMemory::open(
            dir.path().join("m"),
            "h",
            crate::MemoryLimits::default(),
            vec![],
        )
        .unwrap(),
    );
    let cap = budget();
    for sabotage in [false, true] {
        let path = dir
            .path()
            .join(if sabotage { "callback" } else { "intent" });
        let receipt_path = super::super::receipts::path(&path, 1);
        let calls = Arc::new(AtomicUsize::new(0));
        let port: Arc<dyn ModelPort> = if sabotage {
            Arc::new(Sabotage(receipt_path.clone()))
        } else {
            std::fs::create_dir(&receipt_path).unwrap();
            Arc::new(Script {
                calls: calls.clone(),
                fail: false,
                invalid: false,
            })
        };
        let mut journal = EpochJournal::open(&path).unwrap();
        assert!(
            prepare_retreat_epoch(
                &mut journal,
                port,
                memory.clone(),
                inputs(
                    &archive,
                    &checkpoint,
                    &dossier,
                    &epoch,
                    GenerationMode::Direct,
                    &cap
                )
            )
            .await
            .is_err()
        );
        assert!(journal.prepared(1).is_none());
        if sabotage {
            std::fs::remove_dir(&receipt_path).unwrap();
            std::fs::rename(receipt_path.with_extension("saved"), &receipt_path).unwrap();
            assert_eq!(
                journal.generation_receipts(1).unwrap().unwrap().calls[0].status,
                Status::Pending
            );
            drop(journal);
            let mut reopened = EpochJournal::open(path).unwrap();
            assert!(
                prepare_retreat_epoch(
                    &mut reopened,
                    Arc::new(Script {
                        calls: calls.clone(),
                        fail: false,
                        invalid: false
                    }),
                    memory.clone(),
                    inputs(
                        &archive,
                        &checkpoint,
                        &dossier,
                        &epoch,
                        GenerationMode::Direct,
                        &cap
                    )
                )
                .await
                .is_err()
            );
        }
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
}
