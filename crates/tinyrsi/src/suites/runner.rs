//! Isolated coordinator trials and host-metered artifact production.
use super::PrivateTask;
use crate::{
    ArtifactFuture, ArtifactProducer, ArtifactSubmission, Blueprint, BlueprintNode, CandidateState,
    Error, EvalRun, HostVerifier, ModelPort, ModelRequest, NodeStatus, Result, SnapshotMemoryStore,
    VerificationBinding, VerifiedRunner, Verifier, VerifierDeclaration, VerifierKind,
    VerifierRegistry,
};
use crate::{Budget, QualityRubric, VerifierTier};
use std::fmt::Write;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tinyhivemind_hives::{
    AgentRegistration, Coordinator, CoordinatorOptions, Destination, EpisodePhase, HiveInfo,
    MemoryStorage, RetentionPolicy, SendMessage, TurnRequest,
};
use tinyhivemind_lang::{LoweredHive, LoweredSeat, RecallMoment};
#[derive(Clone)]
pub(super) struct RunOptions {
    pub model: Arc<dyn ModelPort>,
    pub max_output_tokens: u64,
    pub timeout_ms: u64,
    pub budget: u64,
    pub seed: u64,
}
#[derive(Clone)]
pub(super) struct Configuration {
    pub hive: LoweredHive,
    pub memory: BTreeMap<String, BTreeMap<String, String>>,
}
pub(super) fn configuration(
    state: &CandidateState,
    memory: &SnapshotMemoryStore,
    model_name: Option<&str>,
) -> Result<Configuration> {
    let hive = tinyhivemind_lang::lower(&state.package).map_err(|_| Error::InvalidRecord)?;
    validate(&hive, model_name)?;
    Ok(Configuration {
        hive,
        memory: memory.read(&state.package, &state.memory_watermarks)?,
    })
}
pub(super) fn validate(hive: &LoweredHive, model_name: Option<&str>) -> Result<()> {
    if !hive.topology.is_empty()
        || !hive.referrals.is_empty()
        || hive.seats.is_empty()
        || hive
            .desks
            .iter()
            .any(|desk| serde_json::to_value(desk).is_ok_and(|v| v.get("responder_mode").is_some()))
    {
        return Err(Error::HostFailed);
    }
    for seat in &hive.seats {
        if !seat.mcp.is_empty()
            || !seat.skills.is_empty()
            || !seat.tools.allow.is_empty()
            || !(seat.config.is_null()
                || seat
                    .config
                    .as_object()
                    .is_some_and(serde_json::Map::is_empty))
            || seat
                .model
                .as_deref()
                .is_some_and(|name| Some(name) != model_name)
        {
            return Err(Error::HostFailed);
        }
    }
    let mut supported = hive.policies.clone();
    let defaults = tinyhivemind_lang::Policies::default();
    supported.episode.turn_budget = defaults.episode.turn_budget;
    supported.episode.round_width = defaults.episode.round_width;
    supported.conduct = defaults.conduct;
    if supported != defaults {
        return Err(Error::HostFailed);
    }
    Ok(())
}
pub(crate) fn options(hive: &LoweredHive) -> CoordinatorOptions {
    let settings = &hive.coordinator;
    let mut conduct = settings.conduct_policy;
    conduct.turn_wall = conduct
        .turn_wall
        .min(hive.policies.conduct.turn_wall)
        .min(u64::from(hive.policies.episode.turn_budget));
    conduct.child_turn_wall = conduct
        .child_turn_wall
        .min(hive.policies.conduct.child_turn_wall);
    CoordinatorOptions {
        round_width: settings
            .round_width
            .min(hive.policies.episode.round_width as usize),
        conduct_policy: conduct,
        broadcast_budget: settings.broadcast_budget,
        retention: RetentionPolicy {
            settled_episodes: settings.retention.settled_episodes,
            delivered: settings.retention.delivered,
            interrupted: settings.retention.interrupted,
            pending_per_agent: settings.retention.pending_per_agent,
        },
    }
}
pub(super) fn system(
    hive: &LoweredHive,
    seat: &LoweredSeat,
    memory: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<String> {
    let role = hive
        .candidates
        .iter()
        .find(|c| c.id == seat.id)
        .and_then(|c| c.description.as_deref())
        .unwrap_or("");
    let mut recalled = String::new();
    if let Some(binding) = &seat.memory
        && binding
            .settings
            .recall_at
            .contains(&RecallMoment::SessionStart)
    {
        for reach in &binding.settings.reads {
            for identity in &hive.memory.identities {
                if (identity.namespace == reach.at
                    || (reach.descendants
                        && identity.namespace.starts_with(&format!("{}/", reach.at))))
                    && let Some(entries) = memory.get(&identity.id)
                {
                    for (key, value) in entries {
                        writeln!(recalled, "{key}: {value}").map_err(|_| Error::HostFailed)?;
                    }
                }
            }
        }
        recalled = recalled
            .chars()
            .take(binding.settings.budget_chars)
            .collect();
    }
    let shared = serde_json::to_string(&hive.context).map_err(|_| Error::HostFailed)?;
    let goals = serde_json::to_string(&hive.goals).map_err(|_| Error::HostFailed)?;
    let missions = serde_json::to_string(&hive.missions).map_err(|_| Error::HostFailed)?;
    Ok(format!(
        "Produce only the required artifact bytes. The host alone checks and completes work.\nRole:\n{role}\nSeat:\n{}\nShared context:\n{shared}\nGoals:\n{goals}\nMissions:\n{missions}\nPinned recalled memory:\n{recalled}",
        seat.prompt
    ))
}
pub(super) struct Producer {
    pub model: Arc<dyn ModelPort>,
    pub request: ModelRequest,
    pub meter: Arc<Mutex<Option<(u64, u64)>>>,
    pub timeout_ms: u64,
}
impl ArtifactProducer for Producer {
    fn produce(&self, _: TurnRequest, binding: VerificationBinding) -> ArtifactFuture {
        let model = self.model.clone();
        let request = self.request.clone();
        let meter = self.meter.clone();
        let timeout = self.timeout_ms;
        Box::pin(async move {
            let reply = tokio::time::timeout(
                std::time::Duration::from_millis(timeout),
                model.generate(request.clone()),
            )
            .await
            .map_err(|_| Error::HostFailed)??;
            request.check(&reply)?;
            *meter.lock().map_err(|_| Error::HostFailed)? =
                Some((reply.input_tokens, reply.output_tokens));
            Ok(ArtifactSubmission {
                binding,
                content: reply.content,
            })
        })
    }
}
pub(super) fn blueprint(task: &PrivateTask) -> Result<Blueprint> {
    let kind = match task.verifier {
        HostVerifier::Golden { .. } => VerifierKind::HeldOutReference,
        HostVerifier::Schema { .. } => VerifierKind::Schema,
        HostVerifier::Process(_) => VerifierKind::Execution,
        HostVerifier::Lean { .. } => VerifierKind::Proof,
    };
    Blueprint::new(vec![BlueprintNode {
        id: "artifact".into(),
        statement: "Produce the host-requested artifact".into(),
        owner_role: task.seat.clone(),
        dependencies: vec![],
        verifier: Verifier {
            kind,
            spec: task.checker_stamp.clone(),
        },
        status: NodeStatus::Open,
    }])
}
pub(super) async fn run(
    task: &PrivateTask,
    config: &Configuration,
    limits: RunOptions,
) -> Result<EvalRun> {
    let request = request(task, config, &limits)?;
    let coordinator = Arc::new(
        Coordinator::new(
            "private-eval".into(),
            Arc::new(MemoryStorage::new()),
            options(&config.hive),
        )
        .await
        .map_err(|_| Error::HostFailed)?,
    );
    let meter = Arc::new(Mutex::new(None));
    let producer = Arc::new(Producer {
        model: limits.model.clone(),
        request,
        meter: meter.clone(),
        timeout_ms: limits.timeout_ms,
    });
    let registry = Arc::new(VerifierRegistry::new(vec![VerifierDeclaration {
        node_id: "artifact".into(),
        owner_seat: task.seat.clone(),
        verifier: task.verifier.clone(),
    }])?);
    let runner = Arc::new(VerifiedRunner::new(
        &coordinator,
        producer,
        registry,
        blueprint(task)?,
    )?);
    coordinator
        .register_agent(AgentRegistration {
            agent_id: task.seat.clone(),
            runtime_id: "private-eval".into(),
            runner: runner.clone(),
        })
        .await
        .map_err(|_| Error::HostFailed)?;
    coordinator
        .create_hive(HiveInfo {
            hive_id: "trial".into(),
            name: "Private trial".into(),
            description: None,
            members: vec![task.seat.clone()],
        })
        .await
        .map_err(|_| Error::HostFailed)?;
    coordinator
        .send_as_host(SendMessage {
            message_id: "trial-task".into(),
            sender: String::new(),
            destination: Destination::Hive("trial".into()),
            body: task.prompt.clone(),
            thread: None,
            only_for: vec![],
            starters: vec![task.seat.clone()],
        })
        .await
        .map_err(|_| Error::HostFailed)?;
    coordinator
        .run_until_idle()
        .await
        .map_err(|_| Error::HostFailed)?;
    observed_run(task, &coordinator, &runner, &meter, &limits)
}

fn observed_run(
    task: &PrivateTask,
    coordinator: &Coordinator,
    runner: &VerifiedRunner,
    meter: &Mutex<Option<(u64, u64)>>,
    limits: &RunOptions,
) -> Result<EvalRun> {
    let evidence = runner.evidence()?;
    if evidence.len() != 1 {
        return Err(Error::HostFailed);
    }
    let result = &evidence[0];
    if result.passed
        && !coordinator
            .episodes()
            .map_err(|_| Error::HostFailed)?
            .iter()
            .any(|episode| {
                episode.episode_id == result.binding.episode_id
                    && episode.phase == EpisodePhase::Settled
            })
    {
        return Err(Error::HostFailed);
    }
    let usage = meter
        .lock()
        .map_err(|_| Error::HostFailed)?
        .ok_or(Error::HostFailed)?;
    let tokens = usage
        .0
        .checked_add(usage.1)
        .ok_or(Error::InvalidEvaluation)?;
    let tier = match task.verifier {
        HostVerifier::Golden { .. } => VerifierTier::HeldOutReference,
        HostVerifier::Schema { .. } => VerifierTier::Schema,
        HostVerifier::Process(_) => VerifierTier::Execution,
        HostVerifier::Lean { .. } => VerifierTier::Proof,
    };
    let quality = QualityRubric::new(0.5, vec![tier])?.score(
        result.passed,
        tokens,
        &Budget {
            max_tokens: limits.budget,
        },
        &[result.passed],
    )?;
    Ok(EvalRun {
        task: task.id.clone(),
        family: task.family.clone(),
        seed: limits.seed,
        quality,
        tokens,
        false_claims: u64::from(!result.passed),
        tamper: 0,
    })
}

pub(super) fn request(
    task: &PrivateTask,
    config: &Configuration,
    limits: &RunOptions,
) -> Result<ModelRequest> {
    let seat = config
        .hive
        .seats
        .iter()
        .find(|s| s.id == task.seat)
        .ok_or(Error::HostFailed)?;
    ModelRequest::new(
        system(&config.hive, seat, &config.memory)?,
        task.prompt.clone(),
        limits.seed,
        limits.budget,
        limits.max_output_tokens.min(limits.budget),
        limits.timeout_ms,
    )
}
