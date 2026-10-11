//! Real coordinator activation, deployment recovery and cursor regressions.
#![expect(clippy::unwrap_used, reason = "assert fixtures and outcomes")]
use super::*;
use std::sync::{Arc, Mutex};
use tinyhivemind_hives::{
    AgentRunner, MemoryStorage, TurnDisposition, TurnFuture, TurnOutcome, TurnRequest,
};
use tinyhivemind_lang::lineage::{Direction, ObjectiveScore, Verdict, VersionMetadata};
use tinyhivemind_lang::patch::{Operation, Patch, PatchClass};
use tinyhivemind_lang::{LoweredHive, LoweredSeat};

struct Memory;
impl MemorySnapshot for Memory {
    fn restore<'a>(&'a self, _: &'a Package, _: &'a BTreeMap<String, u64>) -> HostFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}
struct Runner(String);
impl AgentRunner for Runner {
    fn run(&self, request: TurnRequest) -> TurnFuture {
        let reply = self.0.clone();
        Box::pin(async move {
            Ok(TurnOutcome {
                session_id: request.session_id.unwrap_or_else(|| "session".into()),
                reply: Some(reply),
                disposition: TurnDisposition::Completed,
            })
        })
    }
}
#[derive(Default)]
struct Factory {
    fail: Mutex<bool>,
    fail_attach: Mutex<bool>,
    sessions: Mutex<Vec<Option<String>>>,
}
impl RunnerFactory for Factory {
    fn validate(&self, _: &LoweredHive) -> Result<()> {
        if *self.fail.lock().unwrap() {
            Err(Error::HostFailed)
        } else {
            Ok(())
        }
    }
    fn attach(&self, _: &Arc<Coordinator>) -> Result<()> {
        if *self.fail_attach.lock().unwrap() {
            Err(Error::HostFailed)
        } else {
            Ok(())
        }
    }
    fn build(
        &self,
        _: &LoweredHive,
        seat: &LoweredSeat,
        session: Option<&str>,
    ) -> Result<Arc<dyn AgentRunner>> {
        self.sessions
            .lock()
            .unwrap()
            .push(session.map(str::to_owned));
        Ok(Arc::new(Runner(seat.prompt.clone())))
    }
}
fn metadata(parent: Option<String>, patch: Patch) -> VersionMetadata {
    VersionMetadata {
        parent,
        patch,
        rationale: "evaluated".into(),
        transcript_refs: vec![],
        scores: BTreeMap::from([(
            "quality".into(),
            ObjectiveScore {
                value: 1.0,
                direction: Direction::Maximize,
            },
        )]),
        verdict: Verdict::Accepted,
        memory_watermarks: BTreeMap::new(),
    }
}
fn archive(path: &Path) -> Archive {
    let mut archive = Archive::open(path).unwrap();
    let mut package = Package::default();
    package
        .constitution
        .permitted_edits
        .push(PatchClass::Policy);
    package.manifest.roles.push(tinyhivemind_lang::Role {
        id: "builder".into(),
        charter: "roles/builder.md".into(),
    });
    package
        .documents
        .insert("roles/builder.md".into(), "Build correctly".into());
    package
        .documents
        .insert("seats/alice.md".into(), "Solve reliably".into());
    package.manifest.seats.push(tinyhivemind_lang::Seat {
        id: "alice".into(),
        label: "Alice".into(),
        role: "builder".into(),
        prompt: "seats/alice.md".into(),
        template: "agent".into(),
        config: serde_json::json!({}),
        ..Default::default()
    });
    package.manifest.desks.push(
        serde_json::from_value(
            serde_json::json!({"id":"work","name":"Work","description":null,"members":["alice"]}),
        )
        .unwrap(),
    );
    archive
        .record(
            package,
            metadata(
                None,
                Patch {
                    class: PatchClass::Policy,
                    operations: vec![],
                },
            ),
        )
        .unwrap();
    archive
}
fn child(archive: &mut Archive) {
    let parent = archive.head().unwrap().id.clone();
    let original = archive.package(&parent).unwrap();
    let old =
        serde_json::to_value(original).unwrap()["manifest"]["policies"]["episode"]["turn_budget"]
            .clone();
    let patch = Patch {
        class: PatchClass::Policy,
        operations: vec![Operation::Json {
            pointer: "/manifest/policies/episode/turn_budget".into(),
            before: Some(old),
            after: Some(serde_json::json!(42)),
        }],
    };
    let package = tinyhivemind_lang::patch::apply(original, &patch)
        .unwrap()
        .candidate;
    archive
        .record(package, metadata(Some(parent), patch))
        .unwrap();
}
#[tokio::test]
async fn activation_is_distinct_from_selection_and_rollback_rebuilds_parent() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let base = archive.head().unwrap().id.clone();
    let mut store = DeploymentStore::open(dir.path().join("deployment")).unwrap();
    let storage = Arc::new(MemoryStorage::new());
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        storage.clone(),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    store.activate(&archive, &mut host).await.unwrap();
    let previous = host.coordinator().unwrap().clone();
    child(&mut archive);
    let next = archive.head().unwrap().id.clone();
    assert_eq!(
        store.checkpoint().deployed_id.as_deref(),
        Some(base.as_str())
    );
    store.activate(&archive, &mut host).await.unwrap();
    assert_eq!(
        store.checkpoint().deployed_id.as_deref(),
        Some(next.as_str())
    );
    assert!(
        previous
            .create_hive(tinyhivemind_hives::HiveInfo {
                hive_id: "stale".into(),
                name: "stale".into(),
                description: None,
                members: vec![]
            })
            .await
            .is_err()
    );
    store.rollback(&mut archive, &mut host).await.unwrap();
    assert_eq!(
        store.checkpoint().deployed_id.as_deref(),
        Some(base.as_str())
    );
    assert_eq!(archive.head().unwrap().id, base);
    // The rolled-back design stays an accepted record and becomes a strike.
    let record = archive.records().iter().find(|r| r.id == next).unwrap();
    assert_eq!(
        record.verdict,
        tinyhivemind_lang::lineage::Verdict::Accepted
    );
    let strikes = archive.penalties().strikes;
    assert_eq!(strikes.len(), 1);
    assert_eq!(strikes[0].kind, crate::StrikeKind::Revoked);
    assert_eq!(strikes[0].epoch, store.checkpoint().epoch);
    assert_eq!(strikes[0].shape, crate::PatchShape::of(&record.patch));
}
#[tokio::test]
async fn failed_activation_preserves_deployed_state_and_pending_recovery_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let path = dir.path().join("deployment");
    let factory = Arc::new(Factory::default());
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        factory.clone(),
        Arc::new(Memory),
    );
    let mut store = DeploymentStore::open(&path).unwrap();
    *factory.fail.lock().unwrap() = true;
    assert_eq!(
        store.activate(&archive, &mut host).await,
        Err(Error::HostFailed)
    );
    assert!(store.checkpoint().deployed_id.is_none());
    assert!(store.checkpoint().pending.is_some());
    drop(store);
    let mut store = DeploymentStore::open(path).unwrap();
    assert_eq!(
        store.activate(&archive, &mut host).await,
        Err(Error::HostFailed)
    );
    *factory.fail.lock().unwrap() = false;
    store.recover(&archive, &mut host).await.unwrap();
    assert!(store.checkpoint().pending.is_none());
}
#[test]
fn deployment_store_excludes_writers_and_rejects_corrupt_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deployment");
    let store = DeploymentStore::open(&path).unwrap();
    assert!(matches!(
        DeploymentStore::open(&path),
        Err(Error::ArchiveLocked)
    ));
    drop(store);
    std::fs::write(&path, "{partial").unwrap();
    assert!(matches!(
        DeploymentStore::open(path),
        Err(Error::ArchiveCorrupt)
    ));
}

fn message(
    id: &str,
    destination: tinyhivemind_hives::Destination,
) -> tinyhivemind_hives::SendMessage {
    tinyhivemind_hives::SendMessage {
        message_id: id.into(),
        sender: "ignored".into(),
        destination,
        body: "private task never in observation".into(),
        thread: None,
        only_for: vec![],
        starters: vec![],
    }
}
#[tokio::test]
async fn restart_retains_sessions_and_observation_references_without_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let path = dir.path().join("deployment");
    let factory = Arc::new(Factory::default());
    let storage = Arc::new(MemoryStorage::new());
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        storage.clone(),
        factory.clone(),
        Arc::new(Memory),
    );
    let mut store = DeploymentStore::open(&path).unwrap();
    store.activate(&archive, &mut host).await.unwrap();
    let coordinator = host.coordinator().unwrap().clone();
    coordinator
        .send_as_host(message(
            "one",
            tinyhivemind_hives::Destination::Agent("alice".into()),
        ))
        .await
        .unwrap();
    coordinator.run_until_idle().await.unwrap();
    let observation = store.observe(&coordinator).unwrap();
    assert_ne!(observation.transcript, Vec::new());
    assert_eq!(store.checkpoint().observations.get(&1), Some(&observation));
    let bytes = std::fs::read_to_string(&path).unwrap();
    assert!(!bytes.contains("private task"));
    drop(store);
    let mut store = DeploymentStore::open(path).unwrap();
    let mut restarted =
        CoordinatorHost::new("runtime".into(), storage, factory.clone(), Arc::new(Memory));
    store.recover(&archive, &mut restarted).await.unwrap();
    assert_eq!(
        factory.sessions.lock().unwrap().last().unwrap().as_deref(),
        Some("session")
    );
    assert_eq!(
        store
            .observe(restarted.coordinator().unwrap())
            .unwrap()
            .transcript,
        Vec::new()
    );
    assert_eq!(store.checkpoint().epoch, 2);
}
#[tokio::test]
async fn pending_deliveries_and_open_episodes_block_activation_before_intent() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let mut store = DeploymentStore::open(dir.path().join("deployment")).unwrap();
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    store.activate(&archive, &mut host).await.unwrap();
    let coordinator = host.coordinator().unwrap().clone();
    coordinator
        .send_as_host(message(
            "queued",
            tinyhivemind_hives::Destination::Agent("alice".into()),
        ))
        .await
        .unwrap();
    child(&mut archive);
    assert_eq!(
        store.activate(&archive, &mut host).await,
        Err(Error::HostFailed)
    );
    assert!(store.checkpoint().pending.is_none());
    coordinator.run_until_idle().await.unwrap();
    coordinator
        .send_as_host(message(
            "episode",
            tinyhivemind_hives::Destination::Hive("work".into()),
        ))
        .await
        .unwrap();
    assert_eq!(
        store.activate(&archive, &mut host).await,
        Err(Error::HostFailed)
    );
    assert!(store.checkpoint().pending.is_none());
}
#[tokio::test]
async fn failed_candidate_activation_preserves_deployment_and_can_compensate() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let mut store = DeploymentStore::open(dir.path().join("deployment")).unwrap();
    let factory = Arc::new(Factory::default());
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        factory.clone(),
        Arc::new(Memory),
    );
    store.activate(&archive, &mut host).await.unwrap();
    let base = store.checkpoint().deployed_id.clone();
    child(&mut archive);
    *factory.fail.lock().unwrap() = true;
    assert_eq!(
        store.activate(&archive, &mut host).await,
        Err(Error::HostFailed)
    );
    assert_eq!(store.checkpoint().deployed_id, base);
    assert!(store.observe(host.coordinator().unwrap()).is_err());
    *factory.fail.lock().unwrap() = false;
    store.recover(&archive, &mut host).await.unwrap();
    assert_eq!(store.checkpoint().deployed_id, base);
    assert_ne!(archive.head().unwrap().id, base.unwrap());
}
#[tokio::test]
async fn sqlite_reopens_actual_runtime_and_archive_deployment() {
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let factory = Arc::new(Factory::default());
    let db = dir.path().join("hives.sqlite");
    let deployment = dir.path().join("deployment");
    let mut store = DeploymentStore::open(&deployment).unwrap();
    let mut host =
        CoordinatorHost::sqlite("runtime".into(), &db, factory.clone(), Arc::new(Memory)).unwrap();
    store.activate(&archive, &mut host).await.unwrap();
    host.coordinator()
        .unwrap()
        .send_as_host(message(
            "sqlite",
            tinyhivemind_hives::Destination::Agent("alice".into()),
        ))
        .await
        .unwrap();
    host.coordinator().unwrap().run_until_idle().await.unwrap();
    drop(host);
    drop(store);
    let mut store = DeploymentStore::open(deployment).unwrap();
    let mut host =
        CoordinatorHost::sqlite("runtime".into(), db, factory, Arc::new(Memory)).unwrap();
    store.recover(&archive, &mut host).await.unwrap();
    assert_ne!(
        host.coordinator().unwrap().read_transcript(None).unwrap(),
        Vec::new()
    );
    assert_eq!(host.coordinator().unwrap().list_agents().unwrap().len(), 1);
}
#[test]
fn stale_temporary_files_do_not_prevent_restart_and_malformed_ids_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deployment");
    std::fs::write(sidecar(&path, "next"), "partial").unwrap();
    let store = DeploymentStore::open(&path).unwrap();
    assert!(!sidecar(&path, "next").exists());
    drop(store);
    let checkpoint = DeploymentCheckpoint {
        deployed_id: Some("bad".into()),
        ..DeploymentCheckpoint::default()
    };
    std::fs::write(
        &path,
        serde_json::to_vec(&Snapshot {
            version: 1,
            checkpoint,
        })
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        DeploymentStore::open(&path),
        Err(Error::ArchiveCorrupt)
    ));
    std::fs::write(&path,r#"{"version":2,"checkpoint":{"epoch":0,"observations":{},"transcript_cursor":null,"settled_episodes":[],"deployed_id":null,"pending":null}}"#).unwrap();
    assert!(matches!(
        DeploymentStore::open(&path),
        Err(Error::ArchiveCorrupt)
    ));
}

#[path = "test_runtime.rs"]
mod runtime;
#[path = "test_store.rs"]
mod store;
