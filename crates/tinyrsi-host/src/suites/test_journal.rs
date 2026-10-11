//! Real controlled-host callbacks across epochs, restart and journal failure.
use super::*;
use tinyhivemind_hives::{Destination, EpisodePhase, SendMessage};

async fn send(coordinator: &tinyhivemind_hives::Coordinator, id: &str) {
    coordinator
        .send_as_host(SendMessage {
            message_id: id.into(),
            sender: String::new(),
            thread: None,
            only_for: vec![],
            starters: vec![],
            destination: Destination::Hive("work".into()),
            body: "private task canary".into(),
        })
        .await
        .unwrap();
}
fn package() -> crate::Package {
    let mut package = state().package;
    package.manifest.desks.push(
        serde_json::from_value(
            serde_json::json!({"id":"work","name":"Work","description":null,"members":["alice"]}),
        )
        .unwrap(),
    );
    package
}
async fn host(
    dir: &std::path::Path,
    model: Arc<dyn ModelPort>,
    journal: Arc<crate::EvidenceJournal>,
) -> crate::CoordinatorHost {
    let memory = SnapshotMemoryStore::open(dir.join("memory"), vec![], 100_000).unwrap();
    let factory = ControlledRunnerFactory::new(
        model,
        memory.clone(),
        task("observed", 99),
        Budget { max_tokens: 10_000 },
        100,
        1000,
    )
    .unwrap()
    .with_evidence_journal(journal);
    let mut host = crate::CoordinatorHost::sqlite(
        "production".into(),
        dir.join("runtime.sqlite"),
        Arc::new(factory),
        Arc::new(memory),
    )
    .unwrap();
    crate::ActivationBackend::restore(&mut host, &package(), &BTreeMap::new())
        .await
        .unwrap();
    host
}
#[tokio::test]
async fn two_offline_epochs_record_actual_usage_and_checks_without_recounting_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evidence.json");
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        bad: false,
    });
    for epoch in 1..=2 {
        let journal = Arc::new(crate::EvidenceJournal::open(&path).unwrap());
        let host = host(dir.path(), model.clone(), journal.clone()).await;
        let c = host.coordinator().unwrap();
        let mut changes = c.subscribe();
        send(c, &format!("task-{epoch}")).await;
        c.run_until_idle().await.unwrap();
        changes.changed().await.unwrap();
        let receipt = journal
            .checkpoint(epoch, c, crate::MemoryHealth::default())
            .unwrap();
        assert_eq!(receipt.turn_ids.len(), 1);
        assert_eq!(receipt.dossier.seat_usage["alice"].turns, 1);
        assert_eq!(receipt.dossier.seat_usage["alice"].input_tokens, 10);
        assert_eq!(receipt.dossier.seat_usage["alice"].output_tokens, 2);
        assert_eq!(receipt.unmetered_turns, Vec::<u64>::new());
        assert_eq!(receipt.observation.episodes.len(), 1);
        assert_eq!(
            receipt.observation.episodes[0].phase,
            crate::ObservedEpisodePhase::Settled
        );
        assert_eq!(
            journal
                .checkpoint(epoch, c, crate::MemoryHealth::default())
                .unwrap(),
            receipt
        );
        for turn in journal.turns().unwrap() {
            assert_eq!(turn.verification.len(), 1);
            assert!(turn.verification[0].passed);
            assert_ne!(
                turn.verification[0].binding.transcript_refs,
                Vec::<u64>::new()
            );
            assert!(!journal.record_usage(turn.id, 10, 2).unwrap());
            assert!(
                !journal
                    .record_verification(turn.id, turn.verification[0].clone())
                    .unwrap()
            );
        }
    }
    assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    let journal = crate::EvidenceJournal::open(&path).unwrap();
    assert_eq!(journal.epochs().unwrap().len(), 2);
    let text = std::fs::read_to_string(path).unwrap();
    assert!(!text.contains("private task canary"));
    assert!(!text.contains("Solve observed private canary"));
    assert!(!text.contains("good"));
}

#[tokio::test]
async fn failed_intent_persistence_prevents_provider_dispatch_and_completion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evidence.json");
    let journal = Arc::new(crate::EvidenceJournal::open(&path).unwrap());
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        bad: false,
    });
    let host = host(dir.path(), model.clone(), journal.clone()).await;
    std::fs::create_dir(dir.path().join("evidence.json.next")).unwrap();
    let c = host.coordinator().unwrap();
    send(c, "task").await;
    let _ = c.run_until_idle().await;
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
    assert_eq!(journal.turns().unwrap(), vec![]);
    assert_ne!(c.episodes().unwrap()[0].phase, EpisodePhase::Settled);
}

struct BlockUsage {
    path: std::path::PathBuf,
}
impl ModelPort for BlockUsage {
    fn generate(&self, _: ModelRequest) -> ModelFuture {
        let path = self.path.clone();
        Box::pin(async move {
            std::fs::create_dir(path).unwrap();
            Ok(ModelReply {
                content: b"good".to_vec(),
                input_tokens: 10,
                output_tokens: 2,
            })
        })
    }
}
#[tokio::test]
async fn failed_usage_persistence_keeps_the_intent_and_prevents_checker_success() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evidence.json");
    let journal = Arc::new(crate::EvidenceJournal::open(&path).unwrap());
    let host = host(
        dir.path(),
        Arc::new(BlockUsage {
            path: dir.path().join("evidence.json.next"),
        }),
        journal.clone(),
    )
    .await;
    let c = host.coordinator().unwrap();
    send(c, "task").await;
    let _ = c.run_until_idle().await;
    let turns = journal.turns().unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].status, crate::RecordedTurnStatus::Open);
    assert!(turns[0].usage.is_none());
    assert_eq!(turns[0].verification, vec![]);
    assert_ne!(c.episodes().unwrap()[0].phase, EpisodePhase::Settled);
}

#[tokio::test]
async fn failed_host_check_keeps_measured_usage_and_failure_after_the_provider_callback() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::EvidenceJournal::open(dir.path().join("evidence.json")).unwrap());
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        bad: true,
    });
    let host = host(dir.path(), model, journal.clone()).await;
    let c = host.coordinator().unwrap();
    send(c, "task").await;
    c.run_until_idle().await.unwrap();
    let receipt = journal
        .checkpoint(1, c, crate::MemoryHealth::default())
        .unwrap();
    assert_eq!(receipt.dossier.failed_nodes, vec!["artifact"]);
    assert_eq!(receipt.dossier.seat_usage["alice"].input_tokens, 10);
    assert_eq!(receipt.dossier.wasted_turns, 1);
    assert!(!journal.turns().unwrap()[0].verification[0].passed);
}

struct Overspent;
impl ModelPort for Overspent {
    fn generate(&self, _: ModelRequest) -> ModelFuture {
        Box::pin(async {
            Ok(ModelReply {
                content: b"good".to_vec(),
                input_tokens: 10,
                output_tokens: 101,
            })
        })
    }
}
#[tokio::test]
async fn usage_survives_a_response_cap_violation_without_giving_checker_credit() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::EvidenceJournal::open(dir.path().join("evidence.json")).unwrap());
    let host = host(dir.path(), Arc::new(Overspent), journal.clone()).await;
    let c = host.coordinator().unwrap();
    send(c, "task").await;
    c.run_until_idle().await.unwrap();
    let receipt = journal
        .checkpoint(1, c, crate::MemoryHealth::default())
        .unwrap();
    assert_eq!(receipt.dossier.seat_usage["alice"].output_tokens, 101);
    assert_eq!(receipt.unmetered_turns, Vec::<u64>::new());
    assert_eq!(journal.turns().unwrap()[0].verification, vec![]);
    assert_ne!(c.episodes().unwrap()[0].phase, EpisodePhase::Settled);
}
