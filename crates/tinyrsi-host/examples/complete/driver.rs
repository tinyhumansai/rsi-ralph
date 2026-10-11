//! Actual observed epochs and model-driven experiment assignments.
use super::fixtures;
use sha2::Digest;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tinyhivemind_hives::{Destination, SendMessage};
use tinyrsi_host::{
    Archive, AttemptEvidence, AttemptStatus, Budget, BudgetedModel, CandidateState,
    ControlledRunnerFactory, CoordinatorEvaluator, CoordinatorHost, DeploymentStore, Dossier,
    DurableRetreatMemory, Epoch, EpochJournal, Error, EvalPolicy, ExperimentAblation,
    ExperimentArm, ExperimentExecutor, ExperimentProtocol, ExperimentReport, GenerationBudget,
    GenerationMode, HEpoch, HostVerifier, JsonlTrace, LoopEvent, MemoryHealth, MemoryLimits,
    ModelPort, NodeStatus, PatchClass, PreparedRetreatEpoch, PrivateSuites, PrivateTask,
    ReasonCode, Result, RetreatEpochInput, RetreatEpochOutcome, SnapshotMemoryStore, StopReason,
    TraceSink, TrialEvidence, TrialRequest, VerifierOutcome, evaluate_retreat_epoch,
    prepare_retreat_epoch, record_epoch, run_experiment,
};
pub(super) const ORIGINAL: &str = "Repair the shared context so the solver can solve assigned tasks. Propose a typed context patch with normalized_h prediction and public dossier references.";
pub(super) const EVOLVED: &str = "Repair only context/method.md stable item method. Replace the instruction to return UNKNOWN with a short general instruction to solve each task and emit exactly its requested artifact, no markdown. Use operation=context,path=context/method.md,item=method,before exactly the public item body including trailing newline,after ending newline. Class=context. Never change manifests, evaluators or private tasks. Ground transcript_refs and prediction.nodes in the public dossier. Predict metric=normalized_h and minimum_gain=0.1. Prefer one atomic operation.";
// Identical public prior history is installed before every arm. Memory-off
// retains the same starting store but its generator cannot recall it.
pub(super) const HISTORY: [&str; 2] = [
    "Public prior outcome: rejected proposals that edit evaluators violate the constitution. Propose only permitted context edits grounded in public transcript references.",
    "Public prior outcome: a context method repair should preserve the stable item identity and exact before body, with a specific normalized_h prediction.",
];
pub(super) fn config_id(context: &str) -> String {
    super::fingerprint(format!("{context}\npublic-history-v1:{HISTORY:?}").as_bytes())
}
pub(super) fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .map_err(|_| Error::HostFailed)
}
pub(super) fn observed(
    root: &Path,
    archive: &Archive,
    model: Arc<dyn ModelPort>,
    memory: SnapshotMemoryStore,
) -> Result<(DeploymentStore, Dossier)> {
    let rt = runtime()?;
    let task = PrivateTask::new(
        "public-probe".into(),
        "harness".into(),
        "solver".into(),
        "Public health probe: return only the lowercase SHA-256 hexadecimal digest of TINYRSI_PUBLIC_PROBE_7B5E91.".into(),
        HostVerifier::Golden {
            expected: format!("{:x}",sha2::Sha256::digest(b"TINYRSI_PUBLIC_PROBE_7B5E91")).into_bytes(),
        },
        vec![1],
    )?;
    let factory = Arc::new(ControlledRunnerFactory::new(
        model,
        memory.clone(),
        task,
        Budget { max_tokens: 16_384 },
        256,
        120_000,
    )?);
    let mut host = CoordinatorHost::sqlite(
        "observation".into(),
        root.join("hives.sqlite"),
        factory.clone(),
        Arc::new(memory),
    )?;
    let mut store = DeploymentStore::open(root.join("deployment.json"))?;
    rt.block_on(store.activate(archive, &mut host))?;
    let coordinator = host.coordinator().ok_or(Error::HostFailed)?;
    rt.block_on(coordinator.send_as_host(SendMessage{message_id:"public-failure".into(),sender:"host".into(),thread:None,only_for:vec![],starters:vec![],destination:Destination::Hive("work".into()),body:"Public health probe: emit the lowercase SHA-256 hex digest of TINYRSI_PUBLIC_PROBE_7B5E91. This public probe contains no private evaluation content.".into()})).map_err(|_|Error::HostFailed)?;
    rt.block_on(coordinator.run_until_idle())
        .map_err(|_| Error::HostFailed)?;
    let observation = store.observe(coordinator)?;
    let outcomes = factory
        .evidence()?
        .into_iter()
        .map(|e| VerifierOutcome {
            node_id: e.binding.node_id,
            status: if e.passed {
                NodeStatus::Verified
            } else {
                NodeStatus::Failed
            },
        })
        .collect::<Vec<_>>();
    let dossier = Dossier::build(
        store.checkpoint().epoch,
        &observation,
        &factory.usage()?,
        &outcomes,
        MemoryHealth::default(),
    );
    println!(
        "observed {} transcript references, {} checker outcomes, {} usage records",
        dossier.transcript_sequences.len(),
        outcomes.len(),
        factory.usage()?.len()
    );
    if dossier.failed_nodes.is_empty() {
        return Err(Error::InvalidEvaluation);
    }
    Ok((store, dossier))
}
pub(super) fn epoch(dossier: &Dossier, evaluator: &CoordinatorEvaluator) -> HEpoch {
    HEpoch {
        epoch: Epoch {
            id: dossier.epoch,
            class: PatchClass::Context,
            transcript_refs: dossier.transcript_sequences.clone(),
            failure_nodes: dossier.failed_nodes.clone(),
            budget: Budget { max_tokens: 16_384 },
            penalty: tinyrsi_host::PenaltyPolicy::default(),
            policy: EvalPolicy::default(),
        },
        policy: fixtures::policy(),
        suite_ids: evaluator.suite_ids(),
    }
}
pub(super) fn generation(count: u32, cap: u64) -> GenerationBudget {
    GenerationBudget {
        total_tokens: cap,
        call_tokens: 131_072,
        output_tokens: if count > 1 { 8192 } else { 2048 },
        timeout_ms: 120_000,
        variations: if count > 1 { 20 } else { 1 },
        generations: 2,
        items_per_variation: if count > 1 { 5 } else { 1 },
        select_top_k: count,
        judge_raters: 1,
        wall_clock_secs: 600,
    }
}
pub(super) struct Trials {
    pub root: PathBuf,
    pub model: Arc<dyn ModelPort>,
    pub suites: PrivateSuites,
    pub index: u64,
    /// Loop trace at `<root>/events.jsonl`; one trial is one trace epoch.
    pub trace: JsonlTrace,
}
impl ExperimentExecutor for Trials {
    /// The scoring crate's port carries only its own errors; a host failure in
    /// this example is reported as one.
    fn execute_trial(
        &mut self,
        request: &TrialRequest,
    ) -> std::result::Result<TrialEvidence, tinyrsi_host::ScoreError> {
        self.run_trial(request)
            .map_err(|_| tinyrsi_host::ScoreError::HostFailed)
    }
}
impl Trials {
    fn run_trial(&mut self, request: &TrialRequest) -> Result<TrialEvidence> {
        self.index += 1;
        println!(
            "starting trial {:02} {:?}/{:?}",
            self.index, request.arm, request.ablation
        );
        let dir = self.root.join(format!("trial-{:02}", self.index));
        std::fs::create_dir_all(&dir).map_err(|_| Error::ArchiveIo)?;
        let mut archive = Archive::open(dir.join("archive.json"))?;
        let baseline = fixtures::founding(&mut archive)?;
        if baseline != request.initial_hive_id {
            return Err(Error::InvalidRecord);
        }
        self.trace.record(LoopEvent::EpochStarted {
            epoch: self.index,
            incumbent: baseline.clone(),
        })?;
        let memory = SnapshotMemoryStore::open(
            dir.join("hive-memory.json"),
            vec!["TINYRSI_PRIVATE_CANARY_2026".into()],
            1_000_000,
        )?;
        memory.seed(&fixtures::package(), &BTreeMap::new(), BTreeMap::new())?;
        let cap = Arc::new(BudgetedModel::new(
            self.model.clone(),
            request.evaluation_cap,
        )?);
        let mut evaluator = CoordinatorEvaluator::new(
            self.suites.clone(),
            cap.clone(),
            memory.clone(),
            256,
            120_000,
        )?;
        let state = CandidateState {
            package: fixtures::package(),
            memory_watermarks: BTreeMap::new(),
        };
        let (initial_anchor, initial_health) = evaluator.evaluate_anchor_seeds(
            &state,
            &request.per_run_budget,
            &request.seeds.evaluation_seeds,
        )?;
        let (prepared, outcome) =
            self.propose(&dir, &mut archive, memory, &mut evaluator, request, &cap)?;
        record_epoch(&mut self.trace, self.index, &archive, &prepared, &outcome)?;
        let selected = archive.head().ok_or(Error::InvalidRecord)?;
        let state = CandidateState {
            package: archive
                .package(&selected.id)
                .ok_or(Error::InvalidRecord)?
                .clone(),
            memory_watermarks: selected.memory_watermarks.clone(),
        };
        let (final_anchor, mut final_health) = evaluator.evaluate_anchor_seeds(
            &state,
            &request.per_run_budget,
            &request.seeds.evaluation_seeds,
        )?;
        let initial_cost: u64 = initial_anchor.iter().map(|r| r.tokens).sum();
        let final_cost: u64 = final_anchor.iter().map(|r| r.tokens).sum();
        final_health.cost_ratio =
            f64::from(u32::try_from(final_cost).map_err(|_| Error::InvalidEvaluation)?)
                / f64::from(u32::try_from(initial_cost).map_err(|_| Error::InvalidEvaluation)?);
        let anchor_cost = initial_cost
            .checked_add(final_cost)
            .ok_or(Error::InvalidEvaluation)?;
        let evaluation_tokens = cap
            .meter()?
            .reserved_tokens
            .checked_sub(anchor_cost)
            .ok_or(Error::InvalidEvaluation)?;
        let status = match outcome.candidates.first().map(|c| c.reason) {
            Some(ReasonCode::Accepted) => AttemptStatus::Accepted,
            Some(ReasonCode::Inconclusive) => AttemptStatus::Inconclusive,
            Some(ReasonCode::HostFailure) | None => AttemptStatus::Failed,
            _ => AttemptStatus::Rejected,
        };
        println!(
            "trial {:02} {:?}/{:?}: {:?}, {} model calls; evaluation reservation {}",
            self.index,
            request.arm,
            request.ablation,
            status,
            prepared.meter.attempted_calls,
            cap.meter()?.reserved_tokens
        );
        Ok(TrialEvidence {
            request: request.clone(),
            attempts: vec![AttemptEvidence {
                status,
                generation_tokens: prepared.meter.reserved_tokens,
                evaluation_tokens,
            }],
            initial_anchor,
            final_anchor,
            initial_health,
            final_health,
        })
    }
}

impl Trials {
    /// Runs the preregistered experiment and closes the loop trace.
    pub(super) fn run(
        &mut self,
        protocol: &ExperimentProtocol,
        baseline: &str,
    ) -> Result<ExperimentReport> {
        let result = run_experiment(protocol, self);
        self.trace.record(LoopEvent::RunFinished {
            reason: if result.is_ok() {
                StopReason::MaxEpochs
            } else {
                StopReason::Error
            },
            incumbent: baseline.to_owned(),
            incumbent_metric: None,
        })?;
        Ok(result?)
    }
    fn propose(
        &mut self,
        dir: &Path,
        archive: &mut Archive,
        memory: SnapshotMemoryStore,
        evaluator: &mut CoordinatorEvaluator,
        request: &TrialRequest,
        cap: &Arc<BudgetedModel>,
    ) -> Result<(PreparedRetreatEpoch, RetreatEpochOutcome)> {
        let (store, dossier) = observed(dir, archive, cap.clone(), memory)?;
        let incumbent = archive.head().ok_or(Error::InvalidRecord)?.id.clone();
        self.trace.record(LoopEvent::Observed {
            epoch: self.index,
            incumbent: incumbent.clone(),
            incumbent_metric: None,
            history: u32::try_from(self.index.saturating_sub(1)).unwrap_or(u32::MAX),
            rejected: u32::try_from(dossier.failed_nodes.len()).unwrap_or(u32::MAX),
        })?;
        let mut hepoch = epoch(&dossier, evaluator);
        hepoch.epoch.budget = request.per_run_budget.clone();
        let checkpoint = store.checkpoint();
        let mode = match request.ablation {
            ExperimentAblation::Full => GenerationMode::Retreat,
            ExperimentAblation::MemoryOff => GenerationMode::RetreatWithoutMemory,
            ExperimentAblation::RetreatOff => GenerationMode::Direct,
        };
        let context = match request.arm {
            ExperimentArm::Original => ORIGINAL,
            ExperimentArm::Evolved => EVOLVED,
        };
        if config_id(context) != request.config_id {
            return Err(Error::InvalidRecord);
        }
        let ids = self.suites.fingerprints();
        if ids["selection"] != request.suites.selection
            || ids["confirmation"] != request.suites.confirmation
            || ids["anchor"] != request.suites.anchor
        {
            return Err(Error::InvalidEvaluation);
        }
        let discovery = Arc::new(DurableRetreatMemory::open(
            dir.join("retreat-memory.json"),
            "experiment",
            MemoryLimits::default(),
            vec!["TINYRSI_PRIVATE_CANARY_2026".into()],
        )?);
        runtime()?
            .block_on(tinybackrooms::Memory::store(
                discovery.as_ref(),
                "project:rsi-experiment/agent:retreat",
                HISTORY.iter().map(|value| (*value).into()).collect(),
            ))
            .map_err(|_| Error::HostFailed)?;
        let mut journal = EpochJournal::open(dir.join("epoch.json"))?;
        let budget = generation(1, request.generation_cap);
        let prepared = runtime()?.block_on(prepare_retreat_epoch(
            &mut journal,
            self.model.clone(),
            discovery.clone(),
            RetreatEpochInput {
                hive_id: "experiment",
                archive,
                checkpoint,
                dossier: &dossier,
                epoch: &hepoch,
                mode,
                budget: &budget,
                orchestrator_context: context,
                generation_seed: request.seeds.proposal_seed,
            },
        ))?;
        if prepared.proposals.len()
            != usize::try_from(request.attempts_per_trial).map_err(|_| Error::InvalidEvaluation)?
        {
            return Err(Error::InvalidEvaluation);
        }
        let outcome = evaluate_retreat_epoch(
            &mut journal,
            archive,
            &hepoch,
            evaluator,
            &mut fixtures::ContextGate,
            &discovery,
        )?;
        Ok((prepared, outcome))
    }
}
