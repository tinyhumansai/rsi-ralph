//! Journal failure, validation and rollback transaction regressions.
use super::*;
#[tokio::test]
async fn initial_and_diverged_selection_cannot_roll_back_live_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let mut store = DeploymentStore::open(dir.path().join("deployment")).unwrap();
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    assert_eq!(
        store.rollback(&mut archive, &mut host).await,
        Err(Error::HostFailed)
    );
    store.activate(&archive, &mut host).await.unwrap();
    assert_eq!(
        store.rollback(&mut archive, &mut host).await,
        Err(Error::NoParent)
    );
    store.activate(&archive, &mut host).await.unwrap();
    child(&mut archive);
    assert_eq!(
        store.rollback(&mut archive, &mut host).await,
        Err(Error::HostFailed)
    );
}
#[tokio::test]
async fn interrupted_rollback_finishes_after_archive_selection_was_durable() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let base = archive.head().unwrap().id.clone();
    child(&mut archive);
    let target = archive.head().unwrap().id.clone();
    let path = dir.path().join("deployment");
    let mut store = DeploymentStore::open(&path).unwrap();
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    store.activate(&archive, &mut host).await.unwrap();
    let mut checkpoint = store.state.clone();
    checkpoint.pending = Some(DeploymentIntent {
        from: Some(target),
        to: base.clone(),
        kind: DeploymentKind::Rollback,
        applied: true,
    });
    store.save(checkpoint).unwrap();
    archive.revert().unwrap();
    drop(store);
    let mut store = DeploymentStore::open(path).unwrap();
    store.recover(&archive, &mut host).await.unwrap();
    assert_eq!(store.checkpoint().deployed_id, Some(base));
    assert!(store.checkpoint().pending.is_none());
}
#[tokio::test]
async fn snapshot_write_failure_never_changes_deployment_or_consumes_observations() {
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let path = dir.path().join("deployment");
    let mut store = DeploymentStore::open(&path).unwrap();
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    std::fs::create_dir(sidecar(&path, "next")).unwrap();
    assert_eq!(
        store.activate(&archive, &mut host).await,
        Err(Error::ArchiveIo)
    );
    assert!(host.coordinator().is_none());
    assert!(store.checkpoint().pending.is_none());
    std::fs::remove_dir(sidecar(&path, "next")).unwrap();
    store.activate(&archive, &mut host).await.unwrap();
    std::fs::create_dir(sidecar(&path, "next")).unwrap();
    assert_eq!(
        store.observe(host.coordinator().unwrap()),
        Err(Error::ArchiveIo)
    );
    assert_eq!(store.checkpoint().epoch, 0);
}
#[test]
fn malformed_intents_epochs_and_identity_watermarks_are_rejected() {
    let mut checkpoint = DeploymentCheckpoint::default();
    checkpoint.observations.insert(1, Observation::default());
    assert_eq!(validate(&checkpoint), Err(Error::ArchiveCorrupt));
    checkpoint.observations.clear();
    checkpoint.settled_episodes.insert(" ".into());
    assert_eq!(validate(&checkpoint), Err(Error::ArchiveCorrupt));
    checkpoint.settled_episodes.clear();
    let id = format!("sha256:{}", "a".repeat(64));
    checkpoint.pending = Some(DeploymentIntent {
        from: Some(id.clone()),
        to: id.clone(),
        kind: DeploymentKind::Activate,
        applied: false,
    });
    assert_eq!(validate(&checkpoint), Err(Error::ArchiveCorrupt));
    checkpoint.pending.as_mut().unwrap().from = None;
    checkpoint.pending.as_mut().unwrap().to = "bad".into();
    assert_eq!(validate(&checkpoint), Err(Error::ArchiveCorrupt));
    let dir = tempfile::tempdir().unwrap();
    let archive = Archive::open(dir.path().join("empty")).unwrap();
    assert!(accepted(&archive, &id).is_err());
    assert!(DeploymentStore::open(dir.path()).is_err());
}

#[tokio::test]
async fn rejected_candidate_never_triggers_runtime_activation() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let parent = archive.head().unwrap().id.clone();
    let mut store = DeploymentStore::open(dir.path().join("deployment")).unwrap();
    let factory = Arc::new(Factory::default());
    let storage = Arc::new(MemoryStorage::new());
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        storage.clone(),
        factory.clone(),
        Arc::new(Memory),
    );
    store.activate(&archive, &mut host).await.unwrap();
    let old =
        tinyhivemind_lang::lineage::content_id(archive.package(&parent).unwrap(), &BTreeMap::new())
            .unwrap();
    let original = archive.package(&parent).unwrap();
    let before =
        serde_json::to_value(original).unwrap()["manifest"]["policies"]["episode"]["turn_budget"]
            .clone();
    let patch = Patch {
        class: PatchClass::Policy,
        operations: vec![Operation::Json {
            pointer: "/manifest/policies/episode/turn_budget".into(),
            before: Some(before),
            after: Some(serde_json::json!(43)),
        }],
    };
    let candidate = tinyhivemind_lang::patch::apply(original, &patch)
        .unwrap()
        .candidate;
    let mut meta = metadata(Some(parent), patch);
    meta.verdict = Verdict::Rejected;
    archive.record(candidate, meta).unwrap();
    *factory.fail.lock().unwrap() = true;
    store.activate(&archive, &mut host).await.unwrap();
    assert_eq!(store.checkpoint().deployed_id, Some(old));
    assert_eq!(archive.rejected().len(), 1);
}
