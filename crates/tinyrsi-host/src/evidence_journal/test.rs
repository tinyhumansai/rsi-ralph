//! Callback replay, interruption, atomic checkpoint and corruption regressions.
#![expect(clippy::unwrap_used, reason = "assert test fixtures and receipts")]
use super::*;
use crate::{Error, MemoryHealth, VerificationBinding, VerificationEvidence};
use std::sync::Arc;
use tinyhivemind_hives::{Coordinator, CoordinatorOptions, MemoryStorage};

/// Supplies a binary checker result bound to the deterministic fixture invocation.
fn evidence() -> VerificationEvidence {
    VerificationEvidence {
        binding: VerificationBinding {
            episode_id: "episode".into(),
            seat_id: "alice".into(),
            node_id: "artifact".into(),
            artifact_id: "artifact-1".into(),
            transcript_refs: vec![],
        },
        artifact_digest: "a".repeat(64),
        checker_identity: Some(format!("sha256:{}", "b".repeat(64))),
        passed: true,
        quality: 1.0,
        exit_code: None,
    }
}
/// Creates a Coordinator whose observer state is controlled by each test.
async fn coordinator() -> Coordinator {
    Coordinator::new(
        "test".into(),
        Arc::new(MemoryStorage::new()),
        CoordinatorOptions::default(),
    )
    .await
    .unwrap()
}

#[tokio::test]
/// Pins exactly-once consumption across callback replay and journal reopen.
async fn restart_and_duplicate_callbacks_preserve_exactly_one_measured_turn_per_epoch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evidence.json");
    let journal = EvidenceJournal::open(&path).unwrap();
    let turn = journal.begin_turn("alice", "episode").unwrap();
    assert!(journal.record_usage(turn, 12, 3).unwrap());
    assert!(!journal.record_usage(turn, 12, 3).unwrap());
    assert!(journal.record_verification(turn, evidence()).unwrap());
    assert!(!journal.record_verification(turn, evidence()).unwrap());
    journal.finish_turn(turn).unwrap();
    drop(journal);
    let journal = EvidenceJournal::open(&path).unwrap();
    let c = coordinator().await;
    let first = journal.checkpoint(1, &c, MemoryHealth::default()).unwrap();
    assert_eq!(first.turn_ids, vec![turn]);
    assert_eq!(first.dossier.seat_usage["alice"].input_tokens, 12);
    let next = journal.begin_turn("alice", "next").unwrap();
    journal.record_usage(next, 20, 4).unwrap();
    journal.finish_turn(next).unwrap();
    assert_eq!(
        journal.checkpoint(1, &c, MemoryHealth::default()).unwrap(),
        first
    );
    let second = journal.checkpoint(2, &c, MemoryHealth::default()).unwrap();
    assert_eq!(second.turn_ids, vec![next]);
    assert_eq!(second.dossier.seat_usage["alice"].input_tokens, 20);
    assert!(!journal.record_usage(turn, 12, 3).unwrap());
    drop(journal);
    let journal = EvidenceJournal::open(path).unwrap();
    assert_eq!(journal.epochs().unwrap(), vec![first, second]);
    assert_eq!(journal.turns().unwrap().len(), 2);
}

#[tokio::test]
/// Distinguishes unknown response cost from a measured zero-token invocation.
async fn interrupted_intents_block_epochs_until_explicitly_retained_as_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evidence.json");
    let journal = EvidenceJournal::open(&path).unwrap();
    let unknown = journal.begin_turn("alice", "episode").unwrap();
    let measured = journal.begin_turn("alice", "episode").unwrap();
    journal.record_usage(measured, 7, 2).unwrap();
    drop(journal);
    let journal = EvidenceJournal::open(path).unwrap();
    let c = coordinator().await;
    assert_eq!(
        journal.checkpoint(1, &c, MemoryHealth::default()),
        Err(Error::EvidencePending)
    );
    assert_eq!(journal.interrupt_open_turns().unwrap(), 2);
    assert_eq!(journal.interrupt_open_turns().unwrap(), 0);
    let receipt = journal.checkpoint(1, &c, MemoryHealth::default()).unwrap();
    assert_eq!(receipt.interrupted_turns, vec![unknown, measured]);
    assert_eq!(receipt.unmetered_turns, vec![unknown]);
    assert_eq!(receipt.dossier.seat_usage["alice"].input_tokens, 7);
    assert_eq!(receipt.dossier.wasted_turns, 1);
    assert_eq!(
        journal.record_usage(unknown, 1, 1),
        Err(Error::InvalidEvidence)
    );
    assert_eq!(journal.finish_turn(unknown), Err(Error::InvalidEvidence));
}

#[test]
/// Rejects rewritten measurements and checker evidence from another invocation.
fn conflicting_usage_checker_bindings_and_unknown_turns_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let journal = EvidenceJournal::open(dir.path().join("evidence")).unwrap();
    assert_eq!(
        journal.begin_turn(" ", "episode"),
        Err(Error::InvalidEvidence)
    );
    assert_eq!(
        journal.begin_turn("alice", " "),
        Err(Error::InvalidEvidence)
    );
    let id = journal.begin_turn("alice", "episode").unwrap();
    journal.record_usage(id, 12, 3).unwrap();
    assert_eq!(journal.record_usage(id, 13, 3), Err(Error::InvalidEvidence));
    assert_eq!(journal.record_usage(99, 1, 1), Err(Error::InvalidEvidence));
    for field in 0..6 {
        let mut bad = evidence();
        match field {
            0 => bad.binding.seat_id = "mallory".into(),
            1 => bad.binding.episode_id = "other".into(),
            2 => bad.quality = 0.0,
            3 => bad.artifact_digest = "not-a-digest".into(),
            4 => bad.binding.transcript_refs = vec![2, 1],
            _ => bad.checker_identity = None,
        }
        assert_eq!(
            journal.record_verification(id, bad),
            Err(Error::InvalidEvidence)
        );
    }
    journal.record_verification(id, evidence()).unwrap();
    let mut conflict = evidence();
    conflict.artifact_digest = "b".repeat(64);
    assert_eq!(
        journal.record_verification(id, conflict),
        Err(Error::InvalidEvidence)
    );
    journal.finish_turn(id).unwrap();
    journal.finish_turn(id).unwrap();
    assert!(!journal.record_usage(id, 12, 3).unwrap());
    assert!(!journal.record_verification(id, evidence()).unwrap());
    assert_eq!(journal.finish_turn(99), Err(Error::InvalidEvidence));
}

#[tokio::test]
/// Keeps uncommitted consumption retryable while freezing successful epoch receipts.
async fn persistence_failure_does_not_consume_turns_or_epoch_and_epoch_replay_is_frozen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evidence");
    let journal = EvidenceJournal::open(&path).unwrap();
    let id = journal.begin_turn("alice", "episode").unwrap();
    std::fs::create_dir(storage::sidecar(&path, "next")).unwrap();
    assert_eq!(journal.record_usage(id, 12, 3), Err(Error::ArchiveIo));
    assert!(journal.turns().unwrap()[0].usage.is_none());
    std::fs::remove_dir(storage::sidecar(&path, "next")).unwrap();
    journal.record_usage(id, 12, 3).unwrap();
    journal.finish_turn(id).unwrap();
    let c = coordinator().await;
    std::fs::create_dir(storage::sidecar(&path, "next")).unwrap();
    assert_eq!(
        journal.checkpoint(1, &c, MemoryHealth::default()),
        Err(Error::ArchiveIo)
    );
    assert_eq!(journal.epochs().unwrap(), vec![]);
    std::fs::remove_dir(storage::sidecar(&path, "next")).unwrap();
    assert_eq!(
        journal
            .checkpoint(1, &c, MemoryHealth::default())
            .unwrap()
            .turn_ids,
        vec![id]
    );
    assert_eq!(
        journal.checkpoint(3, &c, MemoryHealth::default()),
        Err(Error::InvalidEvidence)
    );
    assert_eq!(
        journal.checkpoint(0, &c, MemoryHealth::default()),
        Err(Error::InvalidEvidence)
    );
    let health = MemoryHealth {
        stale_entries: 1,
        ..MemoryHealth::default()
    };
    assert_eq!(
        journal.checkpoint(1, &c, health),
        Err(Error::InvalidEvidence)
    );
}

#[test]
/// Prevents concurrent writers and rejects corrupt state without persisting payloads.
fn writer_lock_corrupt_snapshots_and_private_payloads_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evidence");
    let journal = EvidenceJournal::open(&path).unwrap();
    assert!(matches!(
        EvidenceJournal::open(&path),
        Err(Error::ArchiveLocked)
    ));
    let id = journal.begin_turn("alice", "episode").unwrap();
    journal.record_usage(id, 1, 2).unwrap();
    journal.record_verification(id, evidence()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("content"));
    assert!(!text.contains("prompt"));
    drop(journal);
    for replacement in ["{}", "{\"version\":99}", "not json"] {
        std::fs::write(&path, replacement).unwrap();
        assert!(matches!(
            EvidenceJournal::open(&path),
            Err(Error::ArchiveCorrupt)
        ));
    }
}

#[tokio::test]
/// Replays derived dossier and cursor invariants against targeted snapshot mutations.
async fn replay_rejects_rewritten_identities_dossiers_and_consumption_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let journal = EvidenceJournal::open(dir.path().join("evidence")).unwrap();
    let id = journal.begin_turn("alice", "episode").unwrap();
    journal.record_usage(id, 12, 3).unwrap();
    journal.record_verification(id, evidence()).unwrap();
    journal.finish_turn(id).unwrap();
    journal
        .checkpoint(1, &coordinator().await, MemoryHealth::default())
        .unwrap();
    let original = journal.lock().unwrap().state.clone();
    for field in 0..13 {
        let mut state = original.clone();
        match field {
            0 => state.version = 99,
            1 => state.next_turn = 99,
            2 => state.turns.get_mut(&id).unwrap().id = 99,
            3 => state
                .turns
                .get_mut(&id)
                .unwrap()
                .verification
                .push(evidence()),
            4 => {
                state
                    .turns
                    .get_mut(&id)
                    .unwrap()
                    .usage
                    .as_mut()
                    .unwrap()
                    .seat_id = "other".into();
            }
            5 => {
                state
                    .turns
                    .get_mut(&id)
                    .unwrap()
                    .usage
                    .as_mut()
                    .unwrap()
                    .wasted = true;
            }
            6 => state.epochs.get_mut(&1).unwrap().epoch = 99,
            7 => state.epochs.get_mut(&1).unwrap().turn_ids = vec![id, id],
            8 => state.turns.get_mut(&id).unwrap().status = RecordedTurnStatus::Open,
            9 => {
                state
                    .epochs
                    .get_mut(&1)
                    .unwrap()
                    .dossier
                    .seat_usage
                    .get_mut("alice")
                    .unwrap()
                    .input_tokens += 1;
            }
            10 => state.consumed.clear(),
            11 => state.cursor = Some(99),
            _ => {
                state.settled.insert("other".into());
            }
        }
        assert_eq!(validate(&state), Err(Error::ArchiveCorrupt));
    }
    let turn = original.turns[&id].clone();
    let mut legacy = evidence();
    legacy.checker_identity = None;
    assert!(validate_checker(&turn, &legacy, true).is_ok());
    assert_eq!(
        validate_checker(&turn, &legacy, false),
        Err(Error::InvalidEvidence)
    );
    for field in 0..5 {
        let mut bad = evidence();
        match field {
            0 => bad.binding.node_id.clear(),
            1 => bad.binding.artifact_id.clear(),
            2 => bad.artifact_digest = "A".repeat(64),
            3 => bad.quality = f64::NAN,
            _ => bad.checker_identity = Some(format!("sha256:{}", "g".repeat(64))),
        }
        assert_eq!(
            validate_checker(&turn, &bad, false),
            Err(Error::InvalidEvidence)
        );
    }
}

#[tokio::test]
/// Retains checker failure when accounting arrives later and fails closed on poisoning.
async fn failed_checks_before_usage_retain_failure_and_poisoned_collectors_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(EvidenceJournal::open(dir.path().join("evidence")).unwrap());
    let id = journal.begin_turn("alice", "episode").unwrap();
    let mut failed = evidence();
    failed.passed = false;
    failed.quality = 0.0;
    failed.exit_code = Some(1);
    journal.record_verification(id, failed).unwrap();
    journal.record_usage(id, 4, 2).unwrap();
    journal.finish_turn(id).unwrap();
    let receipt = journal
        .checkpoint(1, &coordinator().await, MemoryHealth::default())
        .unwrap();
    assert_eq!(receipt.dossier.failed_nodes, vec!["artifact"]);
    assert_eq!(receipt.dossier.wasted_turns, 1);
    let other = journal.clone();
    let _ = std::thread::spawn(move || {
        let _lock = other.store.lock().unwrap();
        std::panic::resume_unwind(Box::new("poison collector for failure-path assertion"));
    })
    .join();
    assert_eq!(journal.turns(), Err(Error::HostFailed));
}

#[tokio::test]
/// Rejects cross-epoch turn reassignment even when derived totals remain plausible.
async fn replay_rejects_reassigning_measured_turns_to_different_epochs() {
    let dir = tempfile::tempdir().unwrap();
    let journal = EvidenceJournal::open(dir.path().join("evidence")).unwrap();
    let c = coordinator().await;
    for epoch in 1..=2 {
        let id = journal.begin_turn("alice", "episode").unwrap();
        journal.record_usage(id, epoch * 10, 2).unwrap();
        journal.finish_turn(id).unwrap();
        journal
            .checkpoint(epoch, &c, MemoryHealth::default())
            .unwrap();
    }
    let mut state = journal.lock().unwrap().state.clone();
    for (epoch, id) in [(1, 2), (2, 1)] {
        let receipt = build_epoch(
            epoch,
            crate::Observation::default(),
            &[id],
            &state.turns,
            MemoryHealth::default(),
        )
        .unwrap();
        state.epochs.insert(epoch, receipt);
    }
    assert_eq!(validate(&state), Err(Error::ArchiveCorrupt));
}

#[cfg(unix)]
#[test]
/// Protects target paths while recovering only a provably uncommitted temporary write.
fn storage_rejects_symlinks_directories_and_discards_only_uncommitted_temporary_files() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evidence");
    let destination = dir.path().join("other");
    std::fs::write(&destination, b"private").unwrap();
    symlink(&destination, &path).unwrap();
    assert!(matches!(
        EvidenceJournal::open(&path),
        Err(Error::ArchiveIo)
    ));
    std::fs::remove_file(&path).unwrap();
    symlink(&destination, storage::sidecar(&path, "lock")).unwrap();
    assert!(matches!(
        EvidenceJournal::open(&path),
        Err(Error::ArchiveIo)
    ));
    std::fs::remove_file(storage::sidecar(&path, "lock")).unwrap();
    std::fs::create_dir(storage::sidecar(&path, "next")).unwrap();
    assert!(matches!(
        EvidenceJournal::open(&path),
        Err(Error::ArchiveIo)
    ));
    std::fs::remove_dir(storage::sidecar(&path, "next")).unwrap();
    std::fs::write(storage::sidecar(&path, "next"), b"uncommitted").unwrap();
    drop(EvidenceJournal::open(&path).unwrap());
    assert!(!storage::sidecar(&path, "next").exists());
    assert!(EvidenceJournal::open(dir.path()).is_err());
    std::fs::create_dir(&path).unwrap();
    assert!(matches!(
        EvidenceJournal::open(&path),
        Err(Error::ArchiveIo)
    ));
    assert_eq!(std::fs::read(destination).unwrap(), b"private");
}
