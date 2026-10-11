//! Controlled model runner; only the host can submit episode completion.
use super::{ArtifactSubmission, VerificationBinding, VerificationEvidence, VerifierRegistry};
use crate::Blueprint;
use std::collections::BTreeMap;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
};
use tinyhivemind_hives::{
    AgentRunner, Coordinator, EpisodeAction, TurnDisposition, TurnFuture, TurnOutcome, TurnRequest,
};
type SharedReferences = Arc<Mutex<BTreeMap<(String, String), Vec<u64>>>>;
/// Model artifact future; the model has no completion/coordinator capability.
pub type ArtifactFuture = Pin<Box<dyn Future<Output = crate::Result<ArtifactSubmission>> + Send>>;
/// Host model adapter restricted to artifact production.
pub trait ArtifactProducer: Send + Sync {
    /// Produces an artifact for the host-selected authorization.
    fn produce(&self, request: TurnRequest, binding: VerificationBinding) -> ArtifactFuture;
}
/// Host callback invoked after a real checker and before DAG/completion updates.
pub trait VerificationObserver: Send + Sync {
    /// Persist redacted observed evidence; failure prevents completion.
    /// # Errors
    /// Returns a persistence or identity error, causing the runner to park.
    fn observe(&self, evidence: &VerificationEvidence) -> crate::Result<()>;
}
/// Controlled runner that verifies every artifact before completing its episode.
pub struct VerifiedRunner {
    coordinator: Weak<Coordinator>,
    producer: Arc<dyn ArtifactProducer>,
    registry: Arc<VerifierRegistry>,
    template: Blueprint,
    blueprints: Arc<Mutex<BTreeMap<String, Blueprint>>>,
    evidence: Arc<Mutex<Vec<VerificationEvidence>>>,
    references: SharedReferences,
    observer: Option<Arc<dyn VerificationObserver>>,
}
impl VerifiedRunner {
    /// Builds a runner without granting the producer coordinator access.
    /// # Errors
    /// Rejects mismatched registry owners, nodes and blueprint declarations.
    pub fn new(
        coordinator: &Arc<Coordinator>,
        producer: Arc<dyn ArtifactProducer>,
        registry: Arc<VerifierRegistry>,
        blueprint: Blueprint,
    ) -> crate::Result<Self> {
        if registry.declarations().count() != blueprint.nodes().len()
            || blueprint.nodes().iter().any(|n| {
                registry
                    .declarations
                    .get(&n.id)
                    .is_none_or(|d| d.owner_seat != n.owner_role)
            })
        {
            return Err(crate::Error::InvalidBlueprint);
        }
        Ok(Self {
            coordinator: Arc::downgrade(coordinator),
            producer,
            registry,
            template: blueprint,
            blueprints: Arc::new(Mutex::new(BTreeMap::new())),
            evidence: Arc::new(Mutex::new(vec![])),
            references: Arc::new(Mutex::new(BTreeMap::new())),
            observer: None,
        })
    }
    /// Require a successful host evidence callback before marking a node verified.
    #[must_use]
    pub fn with_observer(mut self, observer: Arc<dyn VerificationObserver>) -> Self {
        self.observer = Some(observer);
        self
    }
    /// Public identity-bound checker results, without private verifier output.
    /// # Errors
    /// Returns a host failure if evidence storage is poisoned.
    pub fn evidence(&self) -> crate::Result<Vec<VerificationEvidence>> {
        Ok(self
            .evidence
            .lock()
            .map_err(|_| crate::Error::HostFailed)?
            .clone())
    }
}
impl AgentRunner for VerifiedRunner {
    fn run(&self, request: TurnRequest) -> TurnFuture {
        let coordinator = self.coordinator.clone();
        let producer = self.producer.clone();
        let registry = self.registry.clone();
        let template = self.template.clone();
        let blueprints = self.blueprints.clone();
        let evidence = self.evidence.clone();
        let references = self.references.clone();
        let observer = self.observer.clone();
        Box::pin(async move {
            let session = request
                .session_id
                .clone()
                .unwrap_or_else(|| format!("verified:{}", request.agent_id));
            let parked = || TurnOutcome {
                session_id: session.clone(),
                reply: Some("host verification incomplete".into()),
                disposition: TurnDisposition::Parked,
            };
            let Some(episode) = &request.episode else {
                return Ok(parked());
            };
            let Ok(refs) = references_for(&references, &coordinator, &request, &episode.episode_id)
            else {
                return Ok(parked());
            };
            let node = {
                let Ok(mut guard) = blueprints.lock() else {
                    return Ok(parked());
                };
                let dag = guard.entry(episode.episode_id.clone()).or_insert(template);
                dag.ready()
                    .into_iter()
                    .find(|n| n.owner_role == request.agent_id)
                    .map(|n| n.id.clone())
            };
            let Some(node) = node else {
                let complete = blueprints
                    .lock()
                    .ok()
                    .and_then(|dags| dags.get(&episode.episode_id).map(Blueprint::is_complete))
                    .unwrap_or(false);
                if !complete {
                    return Ok(parked());
                }
                return complete_turn(&coordinator, &request, &episode.episode_id, session).await;
            };
            let binding = VerificationBinding {
                episode_id: episode.episode_id.clone(),
                seat_id: request.agent_id.clone(),
                node_id: node.clone(),
                artifact_id: format!("{}:{node}", episode.episode_id),
                transcript_refs: refs,
            };
            let Ok(artifact) = producer.produce(request.clone(), binding.clone()).await else {
                return Ok(parked());
            };
            let Ok(result) = registry.verify(&binding, &artifact) else {
                return Ok(parked());
            };
            let passed = result.passed;
            if let Ok(mut log) = evidence.lock() {
                log.push(result.clone());
            } else {
                return Ok(parked());
            }
            if observer
                .as_ref()
                .is_some_and(|sink| sink.observe(&result).is_err())
            {
                return Ok(parked());
            }
            let complete = {
                let Ok(mut dags) = blueprints.lock() else {
                    return Ok(parked());
                };
                let Some(dag) = dags.get_mut(&binding.episode_id) else {
                    return Ok(parked());
                };
                if dag.report(&node, passed).is_err() {
                    return Ok(parked());
                }
                dag.is_complete()
            };
            if !complete {
                return Ok(parked());
            }
            complete_turn(&coordinator, &request, &binding.episode_id, session).await
        })
    }
}

impl std::fmt::Debug for VerifiedRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifiedRunner").finish_non_exhaustive()
    }
}

fn references_for(
    cache: &SharedReferences,
    coordinator: &Weak<Coordinator>,
    request: &TurnRequest,
    episode: &str,
) -> crate::Result<Vec<u64>> {
    let mut cache = cache.lock().map_err(|_| crate::Error::HostFailed)?;
    let seen = cache
        .entry((episode.to_owned(), request.agent_id.clone()))
        .or_default();
    seen.extend(request.messages.iter().map(|m| m.sequence));
    let coordinator = coordinator.upgrade().ok_or(crate::Error::HostFailed)?;
    let status = coordinator
        .episodes()
        .map_err(|_| crate::Error::HostFailed)?
        .into_iter()
        .find(|e| e.episode_id == episode)
        .ok_or(crate::Error::InvalidBlueprint)?;
    seen.extend(
        coordinator
            .read_transcript(None)
            .map_err(|_| crate::Error::HostFailed)?
            .into_iter()
            .filter(|m| {
                (m.episode_id.as_deref() == Some(episode) || m.sequence == status.opened_at)
                    && (m.only_for.is_empty() || m.only_for.contains(&request.agent_id))
                    && match &m.destination {
                        tinyhivemind_hives::Destination::Hive(id) => id == &status.hive_id,
                        tinyhivemind_hives::Destination::Agent(id) => id == &request.agent_id,
                    }
            })
            .map(|m| m.sequence),
    );
    seen.sort_unstable();
    seen.dedup();
    Ok(seen.clone())
}
async fn complete_turn(
    coordinator: &Weak<Coordinator>,
    request: &TurnRequest,
    episode: &str,
    session: String,
) -> tinyhivemind_hives::Result<TurnOutcome> {
    let Some(coordinator) = coordinator.upgrade() else {
        return Ok(TurnOutcome {
            session_id: session,
            reply: Some("host verification incomplete".into()),
            disposition: TurnDisposition::Parked,
        });
    };
    coordinator
        .submit_action(
            &request.agent_id,
            episode,
            EpisodeAction::Complete {
                body: "host verified required artifacts".into(),
            },
        )
        .await?;
    Ok(TurnOutcome {
        session_id: session,
        reply: None,
        disposition: TurnDisposition::Completed,
    })
}
