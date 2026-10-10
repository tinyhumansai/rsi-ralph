//! Immutable private draws evaluated inside real, isolated coordinator episodes.
mod factory;
mod fingerprint;
pub use factory::ControlledRunnerFactory;
mod runner;
pub(crate) use runner::options as coordinator_options;
mod types;
use crate::{
    Budget, CandidateState, Error, EvalPair, EvalRun, EvalStage, Evaluator, HEvalPair, HEvaluator,
    ModelPort, Patch, Result, ScoreHealth, SnapshotMemoryStore,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
pub use types::{PrivateSuites, PrivateTask};
/// A synchronous orchestrator bridge owning an isolated asynchronous executor.
///
/// Call evaluation from a nonasync host thread. Every task/seed/arm creates fresh
/// coordinator storage, sessions, verifier state and owned pinned memory maps.
pub struct CoordinatorEvaluator {
    suites: PrivateSuites,
    model: Arc<dyn ModelPort>,
    memory: SnapshotMemoryStore,
    runtime: Option<tokio::runtime::Runtime>,
    max_output_tokens: u64,
    timeout_ms: u64,
    model_name: Option<String>,
}
impl std::fmt::Debug for CoordinatorEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoordinatorEvaluator")
            .field("suites", &self.suites)
            .finish_non_exhaustive()
    }
}
impl Drop for CoordinatorEvaluator {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
impl CoordinatorEvaluator {
    /// Create a bridge with immutable draws and model-call limits.
    /// # Errors
    /// Rejects zero limits, an async caller context or executor initialization failure.
    pub fn new(
        suites: PrivateSuites,
        model: Arc<dyn ModelPort>,
        memory: SnapshotMemoryStore,
        max_output_tokens: u64,
        timeout_ms: u64,
    ) -> Result<Self> {
        if max_output_tokens == 0
            || timeout_ms == 0
            || tokio::runtime::Handle::try_current().is_ok()
        {
            return Err(Error::HostFailed);
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::HostFailed)?;
        Ok(Self {
            suites,
            model,
            memory,
            runtime: Some(runtime),
            max_output_tokens,
            timeout_ms,
            model_name: None,
        })
    }
    /// Bind declared seat model names to the configured model port.
    /// # Errors
    /// Rejects an empty model name.
    pub fn with_model_name(mut self, name: String) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(Error::InvalidProposal);
        }
        self.model_name = Some(name);
        Ok(self)
    }
    /// Opaque identities with the exact three stage names required by `HEpoch`.
    #[must_use]
    pub fn suite_ids(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("Screen".into(), self.suites.fingerprints["screen"].clone()),
            (
                "Selection".into(),
                self.suites.fingerprints["selection"].clone(),
            ),
            (
                "Confirmation".into(),
                self.suites.fingerprints["confirmation"].clone(),
            ),
        ])
    }
    /// Run the separate anchor draw twice against the same pinned design.
    /// # Errors
    /// Returns host execution, snapshot, configuration or cap failures.
    pub fn evaluate_anchor(
        &mut self,
        state: &CandidateState,
        budget: &Budget,
    ) -> Result<HEvalPair> {
        self.paired(state, state, "anchor", budget)
    }
    /// Opaque identity of the separately frozen anchor suite.
    #[must_use]
    pub fn anchor_suite_id(&self) -> String {
        self.suites.fingerprints["anchor"].clone()
    }
    /// Execute one arm of the entire frozen anchor draw.
    /// # Errors
    /// Returns an error for host execution, snapshot or budget failures.
    pub fn evaluate_anchor_once(
        &mut self,
        state: &CandidateState,
        budget: &Budget,
    ) -> Result<(Vec<EvalRun>, ScoreHealth)> {
        let seeds = self.suites.draws["anchor"]
            .iter()
            .flat_map(|task| task.seeds.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        self.evaluate_anchor_seeds(state, budget, &seeds)
    }
    /// Execute a specified subset of the already frozen anchor seeds once.
    ///
    /// The suite identity stays unchanged; returned runs retain exact selected seeds.
    /// # Errors
    /// Rejects unknown/duplicate seeds, missing task coverage or execution failures.
    pub fn evaluate_anchor_seeds(
        &mut self,
        state: &CandidateState,
        budget: &Budget,
        seeds: &[u64],
    ) -> Result<(Vec<EvalRun>, ScoreHealth)> {
        if budget.max_tokens == 0 || tokio::runtime::Handle::try_current().is_ok() {
            return Err(Error::HostFailed);
        }
        let requested = seeds.iter().copied().collect::<BTreeSet<_>>();
        let tasks = &self.suites.draws["anchor"];
        let available = tasks
            .iter()
            .flat_map(|t| t.seeds.iter().copied())
            .collect::<BTreeSet<_>>();
        if requested.is_empty()
            || requested.len() != seeds.len()
            || !requested.is_subset(&available)
            || tasks
                .iter()
                .any(|t| t.seeds.iter().all(|seed| !requested.contains(seed)))
        {
            return Err(Error::InvalidEvaluation);
        }
        let config = runner::configuration(state, &self.memory, self.model_name.as_deref())?;
        let mut jobs = vec![];
        for task in tasks {
            if fingerprint::checker(&task.verifier)? != task.checker_stamp {
                return Err(Error::InvalidEvaluation);
            }
            for seed in &task.seeds {
                if requested.contains(seed) {
                    jobs.push((task.clone(), config.clone(), self.limits(*seed, budget)));
                }
            }
        }
        let runs = self.execute(jobs)?;
        let health = health(&runs, 1.0)?;
        Ok((runs, health))
    }
    fn limits(&self, seed: u64, budget: &Budget) -> runner::RunOptions {
        runner::RunOptions {
            model: self.model.clone(),
            max_output_tokens: self.max_output_tokens,
            timeout_ms: self.timeout_ms,
            budget: budget.max_tokens,
            seed,
        }
    }
    fn execute(
        &self,
        jobs: Vec<(PrivateTask, runner::Configuration, runner::RunOptions)>,
    ) -> Result<Vec<EvalRun>> {
        // Check every input/output reservation before any arm reaches the provider.
        for (task, config, limits) in &jobs {
            runner::request(task, config, limits)?;
        }
        self.runtime
            .as_ref()
            .ok_or(Error::HostFailed)?
            .block_on(async move {
                let mut jobs = jobs.into_iter().enumerate();
                let mut completed = vec![];
                loop {
                    let mut pending = tokio::task::JoinSet::new();
                    for (index, (task, config, limits)) in jobs.by_ref().take(4) {
                        pending.spawn(
                            async move { (index, runner::run(&task, &config, limits).await) },
                        );
                    }
                    if pending.is_empty() {
                        break;
                    }
                    let mut failure = None;
                    while let Some(result) = pending.join_next().await {
                        match result {
                            Ok((index, Ok(run))) => completed.push((index, run)),
                            Ok((_, Err(error))) => {
                                failure.get_or_insert(error);
                            }
                            Err(_) => {
                                failure.get_or_insert(Error::HostFailed);
                            }
                        }
                    }
                    if let Some(error) = failure {
                        return Err(error);
                    }
                }
                completed.sort_by_key(|(index, _)| *index);
                Ok(completed.into_iter().map(|(_, run)| run).collect())
            })
    }
    fn paired(
        &mut self,
        incumbent: &CandidateState,
        candidate: &CandidateState,
        stage: &str,
        budget: &Budget,
    ) -> Result<HEvalPair> {
        if budget.max_tokens == 0 || tokio::runtime::Handle::try_current().is_ok() {
            return Err(Error::HostFailed);
        }
        let tasks = self
            .suites
            .draws
            .get(stage)
            .ok_or(Error::InvalidEvaluation)?;
        let left = runner::configuration(incumbent, &self.memory, self.model_name.as_deref())?;
        let right = runner::configuration(candidate, &self.memory, self.model_name.as_deref())?;
        let mut jobs = vec![];
        for task in tasks {
            if fingerprint::checker(&task.verifier)? != task.checker_stamp {
                return Err(Error::InvalidEvaluation);
            }
            for seed in &task.seeds {
                let limits = self.limits(*seed, budget);
                jobs.push((task.clone(), left.clone(), limits.clone()));
                jobs.push((task.clone(), right.clone(), limits));
            }
        }
        let runs = self.execute(jobs)?;
        let incumbent_runs: Vec<_> = runs.iter().step_by(2).cloned().collect();
        let candidate_runs: Vec<_> = runs.into_iter().skip(1).step_by(2).collect();
        let left_health = health(&incumbent_runs, 1.0)?;
        let left_cost: u64 = incumbent_runs
            .iter()
            .map(|r| r.tokens)
            .try_fold(0u64, u64::checked_add)
            .ok_or(Error::InvalidEvaluation)?;
        let right_cost: u64 = candidate_runs
            .iter()
            .map(|r| r.tokens)
            .try_fold(0u64, u64::checked_add)
            .ok_or(Error::InvalidEvaluation)?;
        let ratio = if left_cost == 0 {
            if right_cost != 0 {
                return Err(Error::InvalidEvaluation);
            }
            1.0
        } else {
            count_float(right_cost)? / count_float(left_cost)?
        };
        Ok(HEvalPair {
            pair: EvalPair {
                incumbent: incumbent_runs,
                candidate: candidate_runs.clone(),
            },
            incumbent_health: left_health,
            candidate_health: health(&candidate_runs, ratio)?,
            suite_id: self.suites.fingerprints[stage].clone(),
        })
    }
}
impl Evaluator for CoordinatorEvaluator {
    fn pin_memory(
        &mut self,
        incumbent: &CandidateState,
        patch: &Patch,
    ) -> Result<BTreeMap<String, u64>> {
        self.memory.pin(incumbent, patch)
    }
    fn evaluate(
        &mut self,
        incumbent: &CandidateState,
        candidate: &CandidateState,
        stage: EvalStage,
        budget: &Budget,
    ) -> Result<EvalPair> {
        Ok(self.evaluate_h(incumbent, candidate, stage, budget)?.pair)
    }
}
impl HEvaluator for CoordinatorEvaluator {
    fn evaluate_h(
        &mut self,
        incumbent: &CandidateState,
        candidate: &CandidateState,
        stage: EvalStage,
        budget: &Budget,
    ) -> Result<HEvalPair> {
        self.paired(
            incumbent,
            candidate,
            match stage {
                EvalStage::Screen => "screen",
                EvalStage::Selection => "selection",
                EvalStage::Confirmation => "confirmation",
            },
            budget,
        )
    }
}
// Convert the two exact 32-bit halves without lossy integer casts.
fn count_float(value: u64) -> Result<f64> {
    let high = u32::try_from(value >> 32).map_err(|_| Error::InvalidEvaluation)?;
    let low = u32::try_from(value & u64::from(u32::MAX)).map_err(|_| Error::InvalidEvaluation)?;
    Ok(f64::from(high) * 4_294_967_296.0 + f64::from(low))
}
fn health(runs: &[EvalRun], cost_ratio: f64) -> Result<ScoreHealth> {
    if runs.is_empty() {
        return Err(Error::InvalidEvaluation);
    }
    let false_claims = runs.iter().filter(|r| r.false_claims > 0).count();
    let tamper = runs.iter().filter(|r| r.tamper > 0).count();
    Ok(ScoreHealth {
        false_claim_rate: count_float(
            false_claims
                .try_into()
                .map_err(|_| Error::InvalidEvaluation)?,
        )? / count_float(
            runs.len()
                .try_into()
                .map_err(|_| Error::InvalidEvaluation)?,
        )?,
        tamper_rate: count_float(tamper.try_into().map_err(|_| Error::InvalidEvaluation)?)?
            / count_float(
                runs.len()
                    .try_into()
                    .map_err(|_| Error::InvalidEvaluation)?,
            )?,
        cost_ratio,
    })
}
#[cfg(test)]
mod test;
