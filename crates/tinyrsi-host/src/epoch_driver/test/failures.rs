//! Frozen configuration, public privacy and durable journal rejection paths.
use super::*;

#[test]
fn rejects_invalid_generation_caps_and_corrupt_or_locked_journals() {
    let dossier = Dossier::build(
        1,
        &Observation::default(),
        &[],
        &[],
        MemoryHealth::default(),
    );
    for bad in [0, 300_001] {
        let mut cap = budget();
        cap.timeout_ms = bad;
        assert!(
            normalized_retreat_request(
                "h",
                &dossier,
                &Package::default(),
                &[],
                GenerationMode::Direct,
                &cap
            )
            .is_err()
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal");
    let journal = EpochJournal::open(&path).unwrap();
    assert!(matches!(
        EpochJournal::open(&path),
        Err(Error::ArchiveLocked)
    ));
    assert!(journal.prepared(1).is_none());
    assert!(journal.outcome(1).is_none());
    assert!(journal.generation_meter(1).is_none());
    assert!(journal.generation_invalid_items(1).is_none());
    assert!(format!("{journal:?}").contains("EpochJournal"));
    drop(journal);
    std::fs::write(&path, b"{}").unwrap();
    assert!(matches!(
        EpochJournal::open(&path),
        Err(Error::ArchiveCorrupt)
    ));
    std::fs::write(&path, br#"{"version":2,"entries":{}}"#).unwrap();
    assert!(matches!(
        EpochJournal::open(&path),
        Err(Error::ArchiveCorrupt)
    ));
}

#[tokio::test]
async fn seed_and_instructions_are_actual_inputs_and_immutable_on_restart() {
    struct Checked;
    impl ModelPort for Checked {
        fn generate(&self, request: crate::ModelRequest) -> crate::ModelFuture {
            assert_eq!(request.seed(), 7001);
            assert!(
                request
                    .prompt()
                    .contains("Frozen public experiment instructions")
            );
            Box::pin(async {
                Ok(crate::ModelReply {
                    content: serde_json::to_vec(&serde_json::json!({"items":[proposal()]}))
                        .unwrap(),
                    input_tokens: 10,
                    output_tokens: 10,
                })
            })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let cap = budget();
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
    let mut input = inputs(
        &archive,
        &checkpoint,
        &dossier,
        &epoch,
        GenerationMode::Direct,
        &cap,
    );
    input.generation_seed = 7001;
    input.orchestrator_context = "Frozen public experiment instructions";
    let prepared = prepare_retreat_epoch(&mut journal, Arc::new(Checked), memory.clone(), input)
        .await
        .unwrap();
    assert_eq!(prepared.epoch, 1);
    for (seed, context) in [
        (7002, "Frozen public experiment instructions"),
        (7001, "Changed instructions"),
    ] {
        let mut input = inputs(
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            GenerationMode::Direct,
            &cap,
        );
        input.generation_seed = seed;
        input.orchestrator_context = context;
        assert!(matches!(
            prepare_retreat_epoch(&mut journal, Arc::new(Checked), memory.clone(), input).await,
            Err(Error::InvalidProposal)
        ));
    }
}

#[tokio::test]
async fn rejects_stale_evidence_and_private_instructions_before_model_call() {
    let dir = tempfile::tempdir().unwrap();
    let (archive, mut checkpoint, mut dossier, mut epoch) = fixtures(&dir.path().join("a"));
    let cap = budget();
    let calls = Arc::new(AtomicUsize::new(0));
    let port = Arc::new(Script {
        calls: calls.clone(),
        fail: false,
        invalid: false,
    });
    let memory = Arc::new(
        DurableRetreatMemory::open(
            dir.path().join("m"),
            "h",
            crate::MemoryLimits::default(),
            vec!["private canary".into()],
        )
        .unwrap(),
    );
    let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
    for context in ["", "private canary"] {
        let mut input = inputs(
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            GenerationMode::Direct,
            &cap,
        );
        input.orchestrator_context = context;
        assert!(
            prepare_retreat_epoch(&mut journal, port.clone(), memory.clone(), input)
                .await
                .is_err()
        );
    }
    dossier.transcript_sequences.push(2);
    assert!(
        prepare_retreat_epoch(
            &mut journal,
            port.clone(),
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
    dossier.transcript_sequences.pop();
    epoch.epoch.failure_nodes.clear();
    assert!(
        prepare_retreat_epoch(
            &mut journal,
            port.clone(),
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
    epoch.epoch.failure_nodes.push("n".into());
    checkpoint.deployed_id = None;
    assert!(
        prepare_retreat_epoch(
            &mut journal,
            port,
            memory,
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
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[cfg(unix)]
#[test]
fn rejects_journal_symlinks_and_oversized_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("j");
    let source = dir.path().join("source");
    std::fs::write(&source, "{}").unwrap();
    std::os::unix::fs::symlink(&source, &path).unwrap();
    assert!(matches!(EpochJournal::open(&path), Err(Error::ArchiveIo)));
    std::fs::remove_file(&path).unwrap();
    std::fs::File::create(&path)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert!(matches!(
        EpochJournal::open(&path),
        Err(Error::ArchiveCorrupt)
    ));
}
