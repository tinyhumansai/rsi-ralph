//! Offline two-epoch observation with durable measured callbacks and real checks.
//!
//! Run `cargo run -p rsi-ralph-host --example observe_epochs`.
//! Artifacts remain in `target/rsi-observe-epochs`; a completed run can reopen.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tinyhivemind_hives::{Destination, SendMessage};
use rsi_ralph_host::{
    ActivationBackend, Budget, ControlledRunnerFactory, CoordinatorHost, Error, EvidenceJournal,
    HostVerifier, MemoryHealth, ModelFuture, ModelPort, ModelReply, ModelRequest, Package,
    PrivateTask, Result, SnapshotMemoryStore,
};

struct OfflineProvider;
impl ModelPort for OfflineProvider {
    /// Returns deterministic offline accounting used by this demonstration only.
    fn generate(&self, _: ModelRequest) -> ModelFuture {
        Box::pin(async {
            // This deterministic adapter supplies its own measured accounting.
            // No hosted-model spend or scientific improvement is claimed.
            Ok(ModelReply {
                content: b"ready".to_vec(),
                input_tokens: 10,
                output_tokens: 2,
            })
        })
    }
}
/// Builds a minimal package with one real task-capable seat and desk.
fn package() -> Result<Package> {
    let mut package = Package::default();
    package
        .manifest
        .roles
        .push(rsi_ralph_host::tinyhivemind_lang::Role {
            id: "builder".into(),
            charter: "roles/builder.md".into(),
        });
    package
        .documents
        .insert("roles/builder.md".into(), "Build correct artifacts".into());
    package.documents.insert(
        "seats/alice.md".into(),
        "Produce the requested artifact".into(),
    );
    package
        .manifest
        .seats
        .push(rsi_ralph_host::tinyhivemind_lang::Seat {
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
        .map_err(|_| Error::InvalidRecord)?,
    );
    Ok(package)
}

/// Runs two observed epochs with SQLite and journal reopen between them.
async fn run(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root).map_err(|_| Error::ArchiveIo)?;
    let path = root.join("evidence.json");
    let completed = EvidenceJournal::open(&path)?.epochs()?.len();
    for index in completed..2 {
        let epoch = u64::try_from(index + 1).map_err(|_| Error::InvalidEvidence)?;
        let journal = Arc::new(EvidenceJournal::open(&path)?);
        let memory = SnapshotMemoryStore::open(root.join("memory.json"), vec![], 100_000)?;
        let task = PrivateTask::new(
            "offline".into(),
            "code".into(),
            "alice".into(),
            "Emit the requested artifact".into(),
            HostVerifier::Golden {
                expected: b"ready".to_vec(),
            },
            vec![1],
        )?;
        let factory = ControlledRunnerFactory::new(
            Arc::new(OfflineProvider),
            memory.clone(),
            task,
            Budget { max_tokens: 10_000 },
            100,
            1000,
        )?
        .with_evidence_journal(journal.clone());
        let mut host = CoordinatorHost::sqlite(
            "offline".into(),
            root.join("runtime.sqlite"),
            Arc::new(factory),
            Arc::new(memory),
        )?;
        host.restore(&package()?, &BTreeMap::new()).await?;
        let coordinator = host.coordinator().ok_or(Error::HostFailed)?;
        let mut changes = coordinator.subscribe();
        coordinator
            .send_as_host(SendMessage {
                message_id: format!("task-{epoch}"),
                sender: "host".into(),
                thread: None,
                only_for: vec![],
                starters: vec![],
                destination: Destination::Hive("work".into()),
                body: "Produce the assigned artifact".into(),
            })
            .await
            .map_err(|_| Error::HostFailed)?;
        coordinator
            .run_until_idle()
            .await
            .map_err(|_| Error::HostFailed)?;
        changes
            .changed()
            .await
            .map_err(|_| Error::ObservationFailed)?;
        let receipt = journal.checkpoint(epoch, coordinator, MemoryHealth::default())?;
        let replay = journal.checkpoint(epoch, coordinator, MemoryHealth::default())?;
        if receipt != replay || receipt.turn_ids.len() != 1 || !receipt.unmetered_turns.is_empty() {
            return Err(Error::InvalidEvidence);
        }
        println!(
            "epoch {epoch}: {} turn, {} input tokens, {} output tokens; repeated checkpoint unchanged",
            receipt.turn_ids.len(),
            receipt.dossier.seat_usage["alice"].input_tokens,
            receipt.dossier.seat_usage["alice"].output_tokens
        );
        // Dropping this scope closes the coordinator, factory and journal;
        // the next epoch reopens the SQLite runtime and evidence file.
    }
    let journal = EvidenceJournal::open(&path)?;
    let epochs = journal.epochs()?;
    let turns = journal.turns()?;
    if epochs.len() != 2
        || turns.len() != 2
        || turns
            .iter()
            .any(|t| t.verification.len() != 1 || !t.verification[0].passed)
    {
        return Err(Error::InvalidEvidence);
    }
    std::fs::write(
        root.join("epochs-public.json"),
        serde_json::to_vec_pretty(&epochs).map_err(|_| Error::ArchiveIo)?,
    )
    .map_err(|_| Error::ArchiveIo)?;
    println!(
        "reopened two epochs and two observed checker passes; artifacts: {}",
        root.display()
    );
    Ok(())
}
#[tokio::main]
/// Writes redacted receipts under the caller-selected persistent output directory.
async fn main() -> Result<()> {
    let root = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/rsi-observe-epochs"), PathBuf::from);
    run(&root).await
}
