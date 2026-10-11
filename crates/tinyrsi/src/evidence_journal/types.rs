//! Redacted durable invocation records and atomic epoch receipts.
use crate::{Dossier, Observation, TurnUsage, VerificationEvidence};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Lifecycle of a host invocation, independent of its verifier result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedTurnStatus {
    /// Invocation intent is durable, but the runner has not returned.
    Open,
    /// Runner returned; failed checks remain failed evidence.
    Finished,
    /// Host stopped the prior runner; external effects may be unknown.
    Interrupted,
}
/// Host-only callback record containing no task or artifact bodies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedTurn {
    /// Monotonic journal invocation ID allocated before dispatch.
    pub id: u64,
    /// Registered seat identity.
    pub seat_id: String,
    /// Active coordinator episode identity.
    pub episode_id: String,
    /// Provider-measured usage; absence means unknown, never zero.
    pub usage: Option<TurnUsage>,
    /// Actual redacted checker evidence, persisted before completion.
    pub verification: Vec<VerificationEvidence>,
    /// Invocation lifecycle.
    pub status: RecordedTurnStatus,
}
/// A replayable observation/dossier transaction consuming exact turn IDs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceEpoch {
    /// Consecutive epoch ID starting at one.
    pub epoch: u64,
    /// Newly consumed invocation IDs, in allocation order.
    pub turn_ids: Vec<u64>,
    /// Invocations whose runner was interrupted.
    pub interrupted_turns: Vec<u64>,
    /// Invocations lacking a durable measured usage callback.
    pub unmetered_turns: Vec<u64>,
    /// Incremental redacted coordinator observation.
    pub observation: Observation,
    /// Deterministic public dossier derived from this epoch's callbacks.
    pub dossier: Dossier,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct State {
    pub version: u32,
    pub next_turn: u64,
    pub turns: BTreeMap<u64, RecordedTurn>,
    pub epochs: BTreeMap<u64, EvidenceEpoch>,
    pub consumed: BTreeSet<u64>,
    pub cursor: Option<u64>,
    pub settled: BTreeSet<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            next_turn: 1,
            turns: BTreeMap::new(),
            epochs: BTreeMap::new(),
            consumed: BTreeSet::new(),
            cursor: None,
            settled: BTreeSet::new(),
        }
    }
}
