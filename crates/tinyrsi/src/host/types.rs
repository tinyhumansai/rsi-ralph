//! Host deployment ports and durable checkpoints.
use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::Arc,
};
use tinyhivemind_hives::AgentRunner;
use tinyhivemind_lang::{LoweredHive, LoweredSeat, Package};
/// Executor-neutral host operation.
pub type HostFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;
/// Factory that implements the complete lowered runtime contract.
///
/// Validation must reject every unsupported model, tool, MCP, skill, routing,
/// context, policy and memory setting before the current writer is fenced.
pub trait RunnerFactory: Send + Sync {
    /// Validate the entire configuration without changing live state.
    /// # Errors
    /// Returns an error for unsupported configuration.
    fn validate(&self, hive: &LoweredHive) -> Result<()>;
    /// Bind prepared runners to the newly claimed coordinator before registration.
    ///
    /// A factory using deferred verifier runners may retain a weak reference;
    /// the backend retains this exact strong handle for the deployed lifetime.
    /// # Errors
    /// Returns an error if binding fails, leaving the deployment intent pending.
    fn attach(&self, _coordinator: &Arc<tinyhivemind_hives::Coordinator>) -> Result<()> {
        Ok(())
    }
    /// Construct an isolated runner, retaining the logical session if present.
    /// # Errors
    /// Returns an error when the configured runner cannot be constructed.
    fn build(
        &self,
        hive: &LoweredHive,
        seat: &LoweredSeat,
        session: Option<&str>,
    ) -> Result<Arc<dyn AgentRunner>>;
}
/// Exact host memory snapshot restoration, implemented by the memory owner.
pub trait MemorySnapshot: Send + Sync {
    /// Validate and atomically restore exactly the named snapshot watermarks.
    ///
    /// Missing snapshots must fail without mutating live memory. Callers invoke
    /// this only after configuration validation and runtime quiescence.
    /// # Errors
    /// Returns an error if identities or watermarks cannot be restored safely.
    fn restore<'a>(
        &'a self,
        package: &'a Package,
        watermarks: &'a BTreeMap<String, u64>,
    ) -> HostFuture<'a, ()>;
}
/// Activation boundary for concrete hosts and deterministic failure tests.
pub trait ActivationBackend: Send {
    /// Confirm that admission is stopped and no running, parked or queued work remains.
    /// # Errors
    /// Returns an error if activation would change in-flight authorization.
    fn quiescent(&self) -> HostFuture<'_, ()>;
    /// Rebuild the selected runtime and restore its exact memory snapshot.
    /// # Errors
    /// Returns an error when activation cannot complete.
    fn apply<'a>(
        &'a mut self,
        package: &'a Package,
        watermarks: &'a BTreeMap<String, u64>,
    ) -> HostFuture<'a, ()>;
    /// Restore a previous runtime after an interrupted activation, or reattach
    /// the exact durable deployment on startup without changing its authorization.
    /// # Errors
    /// Returns an error if compensation cannot complete; the intent stays pending.
    fn restore<'a>(
        &'a mut self,
        package: &'a Package,
        watermarks: &'a BTreeMap<String, u64>,
    ) -> HostFuture<'a, ()>;
}
/// Kind of recoverable deployment transaction.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeploymentKind {
    /// Activate the current accepted design.
    Activate,
    /// Restore the accepted parent and then move archive selection.
    Rollback,
}
/// Durable intent written before any live mutation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DeploymentIntent {
    /// Previously deployed design, absent for first startup.
    pub from: Option<String>,
    /// Exact accepted target design.
    pub to: String,
    /// Intended operation.
    pub kind: DeploymentKind,
    /// True after runtime apply, before durable selection/commit completes.
    pub applied: bool,
}
/// Host-private deployment and observation state, separate from selected lineage.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DeploymentCheckpoint {
    /// Monotonic completed observation epoch.
    pub epoch: u64,
    /// Exact redacted observations retained for restart-safe epoch processing.
    #[serde(default)]
    pub observations: BTreeMap<u64, crate::Observation>,
    /// Last successfully checkpointed transcript reference.
    pub transcript_cursor: Option<u64>,
    /// Episode identities whose terminal state was already consumed.
    pub settled_episodes: BTreeSet<String>,
    /// Actually deployed design; an archive acceptance alone does not change this.
    pub deployed_id: Option<String>,
    /// Incomplete activation; admission must remain closed until recovery.
    pub pending: Option<DeploymentIntent>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snapshot {
    pub version: u32,
    pub checkpoint: DeploymentCheckpoint,
}
