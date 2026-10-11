//! Deferred host-controlled production runners using immutable strong verifiers.
use super::{PrivateTask, runner};
use crate::{
    Budget, Error, ModelPort, ModelRequest, Result, RunnerFactory, SnapshotMemoryStore,
    VerifiedRunner, VerifierDeclaration, VerifierRegistry,
};
use std::sync::{Arc, Mutex, Weak};
use tinyhivemind_hives::{
    AgentRunner, Coordinator, TurnDisposition, TurnFuture, TurnOutcome, TurnRequest,
};
use tinyhivemind_lang::{LoweredHive, LoweredSeat};
/// Artifact-only host factory that installs configured prompts and immutable checkers.
///
/// This adapter supports one declared seat and a one-node host verification DAG.
/// The model has no coordinator, completion tool or verifier mutation capability.
pub struct ControlledRunnerFactory {
    model: Arc<dyn ModelPort>,
    memory: SnapshotMemoryStore,
    task: PrivateTask,
    budget: Budget,
    max_output_tokens: u64,
    timeout_ms: u64,
    model_name: Option<String>,
    coordinator: Arc<Mutex<Weak<Coordinator>>>,
    evidence: Arc<Mutex<Vec<crate::VerificationEvidence>>>,
    usage: Arc<Mutex<Vec<crate::TurnUsage>>>,
}
impl std::fmt::Debug for ControlledRunnerFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlledRunnerFactory")
            .finish_non_exhaustive()
    }
}
impl ControlledRunnerFactory {
    /// Freeze a production task's checker and bounded model settings.
    /// # Errors
    /// Rejects zero caps or invalid task/checker configuration.
    pub fn new(
        model: Arc<dyn ModelPort>,
        memory: SnapshotMemoryStore,
        task: PrivateTask,
        budget: Budget,
        max_output_tokens: u64,
        timeout_ms: u64,
    ) -> Result<Self> {
        if budget.max_tokens == 0 || max_output_tokens == 0 || timeout_ms == 0 {
            return Err(Error::InvalidEvaluation);
        }
        Ok(Self {
            model,
            memory,
            task,
            budget,
            max_output_tokens,
            timeout_ms,
            model_name: None,
            coordinator: Arc::new(Mutex::new(Weak::new())),
            evidence: Arc::new(Mutex::new(vec![])),
            usage: Arc::new(Mutex::new(vec![])),
        })
    }
    /// Identity-bound strong checker evidence without private oracle contents.
    /// # Errors
    /// Returns a host failure if the evidence collector is poisoned.
    pub fn evidence(&self) -> Result<Vec<crate::VerificationEvidence>> {
        Ok(self.evidence.lock().map_err(|_| Error::HostFailed)?.clone())
    }
    /// Actual provider usage measured outside model-produced artifacts.
    /// # Errors
    /// Returns a host failure if the usage collector is poisoned.
    pub fn usage(&self) -> Result<Vec<crate::TurnUsage>> {
        Ok(self.usage.lock().map_err(|_| Error::HostFailed)?.clone())
    }
    /// Allow the declared model name implemented by the supplied model port.
    /// # Errors
    /// Rejects an empty name.
    pub fn with_model_name(mut self, name: String) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(Error::InvalidProposal);
        }
        self.model_name = Some(name);
        Ok(self)
    }
}
impl RunnerFactory for ControlledRunnerFactory {
    fn validate(&self, hive: &LoweredHive) -> Result<()> {
        runner::validate(hive, self.model_name.as_deref())?;
        if hive.seats.len() != 1
            || hive.seats[0].id != self.task.seat
            || super::fingerprint::checker(&self.task.verifier)? != self.task.checker_stamp
        {
            return Err(Error::HostFailed);
        }
        Ok(())
    }
    fn attach(&self, coordinator: &Arc<Coordinator>) -> Result<()> {
        *self.coordinator.lock().map_err(|_| Error::HostFailed)? = Arc::downgrade(coordinator);
        Ok(())
    }
    fn build(
        &self,
        hive: &LoweredHive,
        seat: &LoweredSeat,
        _session: Option<&str>,
    ) -> Result<Arc<dyn AgentRunner>> {
        self.validate(hive)?;
        if seat.id != self.task.seat {
            return Err(Error::HostFailed);
        }
        Ok(Arc::new(DeferredRunner {
            model: self.model.clone(),
            memory: self.memory.clone(),
            task: self.task.clone(),
            hive: hive.clone(),
            seat: seat.clone(),
            budget: self.budget.clone(),
            max_output_tokens: self.max_output_tokens,
            timeout_ms: self.timeout_ms,
            coordinator: self.coordinator.clone(),
            evidence: self.evidence.clone(),
            usage: self.usage.clone(),
        }))
    }
}
struct DeferredRunner {
    model: Arc<dyn ModelPort>,
    memory: SnapshotMemoryStore,
    task: PrivateTask,
    hive: LoweredHive,
    seat: LoweredSeat,
    budget: Budget,
    max_output_tokens: u64,
    timeout_ms: u64,
    coordinator: Arc<Mutex<Weak<Coordinator>>>,
    evidence: Arc<Mutex<Vec<crate::VerificationEvidence>>>,
    usage: Arc<Mutex<Vec<crate::TurnUsage>>>,
}
type Meter = Arc<Mutex<Option<(u64, u64)>>>;
impl DeferredRunner {
    fn prepare(&self, request: &TurnRequest) -> Result<(VerifiedRunner, Meter)> {
        let coordinator = self
            .coordinator
            .lock()
            .map_err(|_| Error::HostFailed)?
            .upgrade()
            .ok_or(Error::HostFailed)?;
        if super::fingerprint::checker(&self.task.verifier)? != self.task.checker_stamp {
            return Err(Error::HostFailed);
        }
        let marks = self.memory.active_watermarks()?;
        let entries = self.memory.read_spec(&self.hive.memory, &marks)?;
        let system = runner::system(&self.hive, &self.seat, &entries)?;
        let visible = request
            .messages
            .iter()
            .map(|m| m.body.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let request = ModelRequest::new(
            system,
            format!("{}\nVisible task messages:\n{visible}", self.task.prompt),
            self.task.seeds[0],
            self.budget.max_tokens,
            self.max_output_tokens,
            self.timeout_ms,
        )?;
        let meter = Arc::new(Mutex::new(None));
        let producer = Arc::new(runner::Producer {
            model: self.model.clone(),
            request,
            meter: meter.clone(),
            timeout_ms: self.timeout_ms,
        });
        let registry = Arc::new(VerifierRegistry::new(vec![VerifierDeclaration {
            node_id: "artifact".into(),
            owner_seat: self.seat.id.clone(),
            verifier: self.task.verifier.clone(),
        }])?);
        Ok((
            VerifiedRunner::new(
                &coordinator,
                producer,
                registry,
                runner::blueprint(&self.task)?,
            )?,
            meter,
        ))
    }
}
impl AgentRunner for DeferredRunner {
    fn run(&self, request: TurnRequest) -> TurnFuture {
        let prepared = self.prepare(&request);
        let evidence = self.evidence.clone();
        let usage = self.usage.clone();
        Box::pin(async move {
            let Ok((verified, meter)) = prepared else {
                return Ok(TurnOutcome {
                    session_id: request
                        .session_id
                        .unwrap_or_else(|| format!("verified:{}", request.agent_id)),
                    reply: Some("host verification incomplete".into()),
                    disposition: TurnDisposition::Parked,
                });
            };
            let seat_id = request.agent_id.clone();
            let outcome = verified.run(request).await?;
            let observed = verified
                .evidence()
                .map_err(|_| tinyhivemind_hives::Error::Poisoned)?;
            let wasted = observed.iter().any(|result| !result.passed);
            evidence
                .lock()
                .map_err(|_| tinyhivemind_hives::Error::Poisoned)?
                .extend(observed);
            if let Some((input_tokens, output_tokens)) = *meter
                .lock()
                .map_err(|_| tinyhivemind_hives::Error::Poisoned)?
            {
                usage
                    .lock()
                    .map_err(|_| tinyhivemind_hives::Error::Poisoned)?
                    .push(crate::TurnUsage {
                        seat_id,
                        input_tokens,
                        output_tokens,
                        wasted,
                    });
            }
            Ok(outcome)
        })
    }
}
