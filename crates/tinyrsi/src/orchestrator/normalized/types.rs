//! Normalized-H epoch, evaluator, and human-approval contracts.

use crate::scoring::ScoringPolicySpec;
use crate::{
    ApplyAt, Budget, CandidateState, Epoch, EvalPair, EvalStage, Evaluator, Proposal, ReasonCode,
    Result, ScoreComparison, ScoreHealth,
};
use std::collections::BTreeMap;

/// Frozen host epoch and normalized scoring configuration.
#[derive(Debug, Clone)]
pub struct HEpoch {
    /// Public evidence, patch class, and hard run caps; raw policy is unused.
    pub epoch: Epoch,
    /// Trusted-host reference anchors and bootstrap policy.
    pub policy: ScoringPolicySpec,
    /// Frozen suite fingerprints keyed by Screen, Selection, Confirmation.
    pub suite_ids: BTreeMap<String, String>,
}
/// Paired outcomes with measured health telemetry and private-suite identity.
#[derive(Debug, Clone)]
pub struct HEvalPair {
    /// Measurements from both arms on identical tasks and seeds.
    pub pair: EvalPair,
    /// Host-calibrated incumbent rates, with cost ratio one.
    pub incumbent_health: ScoreHealth,
    /// Host-calibrated candidate rates and measured relative token cost.
    pub candidate_health: ScoreHealth,
    /// Fingerprint of the immutable private suite used for these runs.
    pub suite_id: String,
}
/// Host runner that exposes normalized evidence while retaining memory pinning.
pub trait HEvaluator: Evaluator {
    /// Execute a frozen private draw and report its verifier and health outcomes.
    ///
    /// # Errors
    /// Returns a host failure if the private draw cannot be executed faithfully.
    fn evaluate_h(
        &mut self,
        incumbent: &CandidateState,
        candidate: &CandidateState,
        stage: EvalStage,
        budget: &Budget,
    ) -> Result<HEvalPair>;
}
/// Human gate that reviews normalized evidence directly.
pub trait HApprovalGate {
    /// Approve the exact content identity and its confirmed normalized gain.
    ///
    /// # Errors
    /// Returns a host failure if an explicit human decision is unavailable.
    fn approve_h(
        &mut self,
        candidate_id: &str,
        proposal: &Proposal,
        comparison: &ScoreComparison,
    ) -> Result<bool>;
}
/// Normalized outcome; approval and host activation remain separate actions.
#[derive(Debug, Clone)]
pub struct HCandidateOutcome {
    /// Durable acceptance or stopping reason.
    pub reason: ReasonCode,
    /// Exact prepared design identity when formalization succeeded.
    pub candidate_id: Option<String>,
    /// Last completed normalized comparison, if any.
    pub comparison: Option<ScoreComparison>,
    /// Host activation granularity, present only for human-approved designs.
    pub apply_at: Option<ApplyAt>,
}
