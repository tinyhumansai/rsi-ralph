//! Typed proposal, evaluation and human-approval ports.
use crate::{Budget, Comparison, EvalPolicy, EvalRun, Package, Patch, PatchClass, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A prediction committed before running the candidate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Prediction {
    /// Objective: `verified_quality` for the raw runner or `normalized_h` for H epochs.
    pub metric: String,
    /// Positive minimum expected quality gain.
    pub minimum_gain: f64,
    /// Public dossier failure node identities this proposal addresses.
    pub nodes: Vec<String>,
}
/// Strict retreat output; the model proposes and the host decides.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// Short description.
    pub title: String,
    /// Atomic language operation with one authority class.
    pub patch: Patch,
    /// Claimed causal mechanism, retained as rationale.
    pub mechanism: String,
    /// Checkable prediction.
    pub prediction: Prediction,
    /// Supporting public transcript sequence numbers.
    pub transcript_refs: Vec<u64>,
    /// Risks the proposer expects.
    pub risk: String,
    /// Expected implementation effort.
    pub effort: String,
}
/// Public evidence and immutable host settings for one epoch.
#[derive(Debug, Clone)]
pub struct Epoch {
    /// Monotonic host epoch label.
    pub id: u64,
    /// Only this authority class may be proposed in this epoch.
    pub class: PatchClass,
    /// Observed public transcript references.
    pub transcript_refs: Vec<u64>,
    /// Observed failed or open blueprint node identities.
    pub failure_nodes: Vec<String>,
    /// Hard per-run budget.
    pub budget: Budget,
    /// Host-selected statistical constraints.
    pub policy: EvalPolicy,
}
/// Cheap-to-expensive evaluation stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalStage {
    /// Small replay and regression screen.
    Screen,
    /// Complete selection suite.
    Selection,
    /// Fresh private tasks and seeds.
    Confirmation,
}
/// Candidate spec plus the exact pinned hive memory evaluated with it.
#[derive(Debug, Clone)]
pub struct CandidateState {
    /// Immutable candidate package.
    pub package: Package,
    /// Persistent memory event watermarks, complete for the package.
    pub memory_watermarks: BTreeMap<String, u64>,
}
/// Paired results; private task contents stay inside the harness.
#[derive(Debug, Clone)]
pub struct EvalPair {
    /// Incumbent measurements.
    pub incumbent: Vec<EvalRun>,
    /// Candidate measurements on the identical draw.
    pub candidate: Vec<EvalRun>,
}
/// Host-owned private task runner. It must enforce caps while executing.
pub trait Evaluator {
    /// Fork learning into an isolated snapshot, without changing live memory.
    ///
    /// # Errors
    /// Returns a host failure if memory cannot be pinned reproducibly.
    fn pin_memory(
        &mut self,
        incumbent: &CandidateState,
        patch: &Patch,
    ) -> Result<BTreeMap<String, u64>>;
    /// Evaluate both states against host-owned verifiers, tasks and seeds.
    ///
    /// # Errors
    /// Returns a host failure for execution or verifier errors.
    fn evaluate(
        &mut self,
        incumbent: &CandidateState,
        candidate: &CandidateState,
        stage: EvalStage,
        budget: &Budget,
    ) -> Result<EvalPair>;
}
/// Explicit human approval boundary; never implemented by the proposer.
pub trait ApprovalGate {
    /// Approve this exact candidate identity after reading its paired evidence.
    ///
    /// # Errors
    /// Returns a host failure if the human decision is unavailable.
    fn approve(
        &mut self,
        candidate_id: &str,
        proposal: &Proposal,
        comparison: &Comparison,
    ) -> Result<bool>;
}
/// Why a candidate stopped or was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReasonCode {
    /// Incomplete or invalid prediction.
    Untestable,
    /// Evidence references are absent from the public dossier.
    Ungrounded,
    /// Another class was requested.
    WrongClass,
    /// Language constitution or verifier protection failed.
    Guard,
    /// The typed patch failed its preconditions or validation.
    Unformalizable,
    /// An earlier proposal already used these exact operations.
    Duplicate,
    /// Paired gain or a hard constraint failed.
    NoGain,
    /// Evidence did not separate improvement from noise.
    Inconclusive,
    /// Improvement did not replicate on fresh draws.
    NotReplicated,
    /// The human declined promotion.
    ApprovalDenied,
    /// Evaluator or approval service failed.
    HostFailure,
    /// Selected, confirmed and human-approved design.
    Accepted,
}
/// Granularity the host must use when activating the selected snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyAt {
    /// Context or learned-memory delta at the next turn boundary.
    NextTurn,
    /// Prompt replacement while preserving the session.
    ReplaceAgent,
    /// Roster operations at a safe session boundary.
    ManageRoster,
    /// Restart the coordinator with the same storage between sessions.
    BetweenSessions,
}
/// Durable outcome; selection never directly mutates a running coordinator.
#[derive(Debug, Clone)]
pub struct CandidateOutcome {
    /// Machine-readable outcome.
    pub reason: ReasonCode,
    /// Design identity when formalization yielded a valid candidate.
    pub candidate_id: Option<String>,
    /// Last completed paired comparison.
    pub comparison: Option<Comparison>,
    /// Activation requested from the host only for accepted candidates.
    pub apply_at: Option<ApplyAt>,
}
