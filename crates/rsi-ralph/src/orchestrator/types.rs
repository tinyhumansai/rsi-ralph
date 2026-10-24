//! Typed proposal, evaluation and human-approval ports.
use crate::{
    Budget, CandidateState, Comparison, EvalPolicy, EvalRun, EvalStage, Patch, PatchClass,
    PenaltyPolicy, Proposal, ReasonCode, Result,
};
use std::collections::BTreeMap;

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
    /// Strikes that cool a patch shape before any evaluation spend.
    pub penalty: PenaltyPolicy,
    /// Host-selected statistical constraints.
    pub policy: EvalPolicy,
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
