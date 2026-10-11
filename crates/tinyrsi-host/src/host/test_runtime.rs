//! Runtime configuration, membership and pre-fencing failure regressions.
use super::*;
#[tokio::test]
async fn roster_rebuild_updates_membership_and_keeps_removed_agents_dormant() {
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let mut package = archive
        .package(&archive.head().unwrap().id)
        .unwrap()
        .clone();
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    host.apply(&package, &BTreeMap::new()).await.unwrap();
    let old = host.coordinator().unwrap().clone();
    let mut bob = package.manifest.seats[0].clone();
    bob.id = "bob".into();
    bob.label = "Bob".into();
    bob.prompt = "seats/bob.md".into();
    package
        .documents
        .insert(bob.prompt.clone(), "New seat".into());
    package.manifest.seats.push(bob);
    package.manifest.desks[0].members = vec!["bob".into()];
    host.apply(&package, &BTreeMap::new()).await.unwrap();
    assert_eq!(
        host.coordinator().unwrap().list_hives().unwrap()[0].members,
        vec!["bob"]
    );
    assert!(old.join_hive("work", "alice").await.is_err());
    package.manifest.desks.clear();
    package.manifest.seats.retain(|s| s.id == "bob");
    host.apply(&package, &BTreeMap::new()).await.unwrap();
    assert_eq!(
        host.coordinator().unwrap().list_hives().unwrap()[0].members,
        Vec::<String>::new()
    );
    assert_eq!(host.coordinator().unwrap().list_agents().unwrap().len(), 2);
}
#[tokio::test]
async fn unsupported_metadata_and_failed_memory_restore_leave_original_writer_unfenced() {
    struct Missing;
    impl MemorySnapshot for Missing {
        fn restore<'a>(
            &'a self,
            _: &'a Package,
            _: &'a BTreeMap<String, u64>,
        ) -> HostFuture<'a, ()> {
            Box::pin(async { Err(Error::HostFailed) })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let mut package = archive
        .package(&archive.head().unwrap().id)
        .unwrap()
        .clone();
    let storage = Arc::new(MemoryStorage::new());
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        storage.clone(),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    host.apply(&package, &BTreeMap::new()).await.unwrap();
    let live = host.coordinator().unwrap().clone();
    package.manifest.desks[0].name = "unsupported rename".into();
    assert_eq!(
        host.apply(&package, &BTreeMap::new()).await,
        Err(Error::HostFailed)
    );
    live.join_hive("work", "alice").await.unwrap();
    package.manifest.desks[0].name = "Work".into();
    let mut replacement = CoordinatorHost::new(
        "runtime".into(),
        storage,
        Arc::new(Factory::default()),
        Arc::new(Missing),
    );
    assert_eq!(
        replacement.apply(&package, &BTreeMap::new()).await,
        Err(Error::HostFailed)
    );
    live.join_hive("work", "alice").await.unwrap();
    assert!(replacement.coordinator().is_none());
}
#[tokio::test]
async fn parked_agents_block_rebuild() {
    use tinyhivemind_hives::{Commit, Storage};
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let package = archive.package(&archive.head().unwrap().id).unwrap();
    let storage = Arc::new(MemoryStorage::new());
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        storage.clone(),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    host.apply(package, &BTreeMap::new()).await.unwrap();
    let mut state = storage.load().await.unwrap();
    let revision = state.revision;
    state.revision += 1;
    state.agents.get_mut("alice").unwrap().parked = true;
    storage
        .commit(Commit {
            expected_revision: revision,
            state: &state,
            appended: &[],
        })
        .await
        .unwrap();
    assert_eq!(host.quiescent().await, Err(Error::HostFailed));
}

#[tokio::test]
async fn membership_reordering_preserves_the_exact_declared_lead_order() {
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let mut package = archive
        .package(&archive.head().unwrap().id)
        .unwrap()
        .clone();
    let mut bob = package.manifest.seats[0].clone();
    bob.id = "bob".into();
    bob.label = "Bob".into();
    bob.prompt = "seats/bob.md".into();
    package.documents.insert(bob.prompt.clone(), "Bob".into());
    package.manifest.seats.push(bob);
    package.manifest.desks[0].members = vec!["alice".into(), "bob".into()];
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    host.apply(&package, &BTreeMap::new()).await.unwrap();
    package.manifest.desks[0].members = vec!["bob".into(), "alice".into()];
    host.apply(&package, &BTreeMap::new()).await.unwrap();
    assert_eq!(
        host.coordinator().unwrap().list_hives().unwrap()[0].members,
        package.manifest.desks[0].members
    );
}

#[tokio::test]
async fn post_fencing_registration_failure_restores_exact_old_memory_and_configuration() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tinyhivemind_hives::{Commit, Storage, StorageFuture, StoredState};
    struct FailsOnce {
        inner: MemoryStorage,
        remaining: AtomicUsize,
    }
    impl Storage for FailsOnce {
        fn load(&self) -> StorageFuture<'_, StoredState> {
            self.inner.load()
        }
        fn commit<'a>(&'a self, commit: Commit<'a>) -> StorageFuture<'a, ()> {
            Box::pin(async move {
                let count = self.remaining.load(Ordering::SeqCst);
                if count > 0 && self.remaining.fetch_sub(1, Ordering::SeqCst) == 1 {
                    return Err(tinyhivemind_hives::Error::Poisoned);
                }
                self.inner.commit(commit).await
            })
        }
    }
    #[derive(Default)]
    struct Images(Mutex<Vec<String>>);
    impl MemorySnapshot for Images {
        fn restore<'a>(
            &'a self,
            package: &'a Package,
            watermarks: &'a BTreeMap<String, u64>,
        ) -> HostFuture<'a, ()> {
            Box::pin(async move {
                self.0
                    .lock()
                    .unwrap()
                    .push(tinyhivemind_lang::lineage::content_id(package, watermarks).unwrap());
                Ok(())
            })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let base = archive.head().unwrap().id.clone();
    let storage = Arc::new(FailsOnce {
        inner: MemoryStorage::new(),
        remaining: AtomicUsize::new(0),
    });
    let images = Arc::new(Images::default());
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        storage.clone(),
        Arc::new(Factory::default()),
        images.clone(),
    );
    let mut store = DeploymentStore::open(dir.path().join("deployment")).unwrap();
    store.activate(&archive, &mut host).await.unwrap();
    let old = host.coordinator().unwrap().clone();
    child(&mut archive);
    let candidate = archive.head().unwrap().id.clone();
    storage.remaining.store(2, Ordering::SeqCst);
    assert_eq!(
        store.activate(&archive, &mut host).await,
        Err(Error::HostFailed)
    );
    assert!(old.join_hive("work", "alice").await.is_err());
    assert!(store.checkpoint().pending.is_some());
    assert_eq!(images.0.lock().unwrap().last(), Some(&candidate));
    store.recover(&archive, &mut host).await.unwrap();
    assert_eq!(images.0.lock().unwrap().last(), Some(&base));
    assert_eq!(store.checkpoint().deployed_id, Some(base));
    assert!(store.checkpoint().pending.is_none());
    host.coordinator()
        .unwrap()
        .send_as_host(message(
            "after-restore",
            tinyhivemind_hives::Destination::Agent("alice".into()),
        ))
        .await
        .unwrap();
    host.coordinator().unwrap().run_until_idle().await.unwrap();
    assert!(format!("{host:?}").contains("active: true"));
    assert!(format!("{store:?}").contains("DeploymentStore"));
}

#[tokio::test]
async fn deferred_runner_binding_failure_is_compensated_after_writer_fencing() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let factory = Arc::new(Factory::default());
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        factory.clone(),
        Arc::new(Memory),
    );
    let mut store = DeploymentStore::open(dir.path().join("deployment")).unwrap();
    store.activate(&archive, &mut host).await.unwrap();
    let base = store.checkpoint().deployed_id.clone();
    let old = host.coordinator().unwrap().clone();
    child(&mut archive);
    *factory.fail_attach.lock().unwrap() = true;
    assert_eq!(
        store.activate(&archive, &mut host).await,
        Err(Error::HostFailed)
    );
    assert!(old.join_hive("work", "alice").await.is_err());
    assert!(store.checkpoint().pending.is_some());
    *factory.fail_attach.lock().unwrap() = false;
    store.recover(&archive, &mut host).await.unwrap();
    assert_eq!(store.checkpoint().deployed_id, base);
    host.coordinator()
        .unwrap()
        .send_as_host(message(
            "bound",
            tinyhivemind_hives::Destination::Agent("alice".into()),
        ))
        .await
        .unwrap();
    host.coordinator().unwrap().run_until_idle().await.unwrap();
}

#[tokio::test]
async fn restart_reattaches_exact_deployed_configuration_before_draining_durable_queue() {
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let storage = Arc::new(MemoryStorage::new());
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        storage.clone(),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    let mut store = DeploymentStore::open(dir.path().join("deployment")).unwrap();
    store.activate(&archive, &mut host).await.unwrap();
    host.coordinator()
        .unwrap()
        .send_as_host(message(
            "restart-queued",
            tinyhivemind_hives::Destination::Agent("alice".into()),
        ))
        .await
        .unwrap();
    drop(host);
    let mut restarted = CoordinatorHost::new(
        "runtime".into(),
        storage,
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    store.recover(&archive, &mut restarted).await.unwrap();
    assert_eq!(
        restarted
            .coordinator()
            .unwrap()
            .run_until_idle()
            .await
            .unwrap()
            .completed,
        1
    );
}

#[tokio::test]
async fn factory_without_deferred_runners_can_use_default_attachment() {
    struct Basic;
    impl RunnerFactory for Basic {
        fn validate(&self, _: &LoweredHive) -> Result<()> {
            Ok(())
        }
        fn build(
            &self,
            _: &LoweredHive,
            _: &LoweredSeat,
            _: Option<&str>,
        ) -> Result<Arc<dyn AgentRunner>> {
            Ok(Arc::new(Runner("basic".into())))
        }
    }
    let coordinator = Arc::new(
        Coordinator::new(
            "basic".into(),
            Arc::new(MemoryStorage::new()),
            tinyhivemind_hives::CoordinatorOptions::default(),
        )
        .await
        .unwrap(),
    );
    Basic.attach(&coordinator).unwrap();
    let package = Package::default();
    let hive = tinyhivemind_lang::lower(&package).unwrap();
    Basic.validate(&hive).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let hive =
        tinyhivemind_lang::lower(archive.package(&archive.head().unwrap().id).unwrap()).unwrap();
    Basic.build(&hive, &hive.seats[0], None).unwrap();
}

#[tokio::test]
async fn attached_runner_settles_real_episodes_and_terminal_evidence_is_consumed_once() {
    use std::sync::Weak;
    struct FactoryWithCompletion(Arc<Mutex<Weak<Coordinator>>>);
    struct CompletingRunner(Arc<Mutex<Weak<Coordinator>>>);
    impl RunnerFactory for FactoryWithCompletion {
        fn validate(&self, _: &LoweredHive) -> Result<()> {
            Ok(())
        }
        fn attach(&self, coordinator: &Arc<Coordinator>) -> Result<()> {
            *self.0.lock().unwrap() = Arc::downgrade(coordinator);
            Ok(())
        }
        fn build(
            &self,
            _: &LoweredHive,
            _: &LoweredSeat,
            _: Option<&str>,
        ) -> Result<Arc<dyn AgentRunner>> {
            Ok(Arc::new(CompletingRunner(self.0.clone())))
        }
    }
    impl AgentRunner for CompletingRunner {
        fn run(&self, request: TurnRequest) -> TurnFuture {
            let coordinator = self.0.lock().unwrap().upgrade().unwrap();
            Box::pin(async move {
                if let Some(episode) = request.episode {
                    coordinator
                        .submit_action(
                            &request.agent_id,
                            &episode.episode_id,
                            tinyhivemind_hives::EpisodeAction::Complete {
                                body: "private verifier evidence".into(),
                            },
                        )
                        .await?;
                }
                Ok(TurnOutcome {
                    session_id: request.session_id.unwrap_or_else(|| "continuing".into()),
                    reply: None,
                    disposition: TurnDisposition::Completed,
                })
            })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let path = dir.path().join("deployment");
    let slot = Arc::new(Mutex::new(Weak::new()));
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        Arc::new(FactoryWithCompletion(slot.clone())),
        Arc::new(Memory),
    );
    let mut store = DeploymentStore::open(&path).unwrap();
    store.activate(&archive, &mut host).await.unwrap();
    let coordinator = host.coordinator().unwrap();
    coordinator
        .send_as_host(message(
            "conduct",
            tinyhivemind_hives::Destination::Hive("work".into()),
        ))
        .await
        .unwrap();
    coordinator.run_until_idle().await.unwrap();
    let observed = store.observe(coordinator).unwrap();
    assert!(
        observed
            .episodes
            .iter()
            .any(|e| e.phase == crate::ObservedEpisodePhase::Settled)
    );
    let id = observed.episodes[0].episode_id.clone();
    assert!(store.checkpoint().settled_episodes.contains(&id));
    drop(store);
    let mut store = DeploymentStore::open(&path).unwrap();
    assert_eq!(store.observe(coordinator).unwrap().episodes, Vec::new());
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("private verifier evidence")
    );
    assert!(slot.lock().unwrap().upgrade().is_some());
    drop(host);
    assert!(slot.lock().unwrap().upgrade().is_none());
}

#[tokio::test]
async fn declared_episode_turn_cap_bounds_real_deployed_scheduler() {
    let dir = tempfile::tempdir().unwrap();
    let archive = archive(&dir.path().join("archive"));
    let mut package = archive
        .package(&archive.head().unwrap().id)
        .unwrap()
        .clone();
    package.manifest.policies.episode.turn_budget = 1;
    let mut host = CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    );
    host.apply(&package, &BTreeMap::new()).await.unwrap();
    let coordinator = host.coordinator().unwrap();
    coordinator
        .send_as_host(message(
            "bounded",
            tinyhivemind_hives::Destination::Hive("work".into()),
        ))
        .await
        .unwrap();
    let report = coordinator.run_until_idle().await.unwrap();
    assert!(report.completed <= 1);
    assert!(
        coordinator
            .episodes()
            .unwrap()
            .iter()
            .all(|episode| matches!(episode.phase, tinyhivemind_hives::EpisodePhase::Failed(_)))
    );
}
