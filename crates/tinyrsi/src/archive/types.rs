//! Persisted archive events and proposal audit evidence.
use crate::ProposalEvent;
use serde::{Deserialize, Serialize};
use tinyhivemind_lang::Package;
use tinyhivemind_lang::lineage::Record;

/// Exact proposal inputs committed before executing a candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalDeclaration {
    /// Host epoch that issued the declaration.
    pub epoch: u64,
    /// Original typed proposal, including prediction, risk and effort.
    pub proposal: crate::Proposal,
}

/// Frozen normalized host configuration committed before candidate execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedDeclarationEvent {
    /// Frozen host epoch identity.
    pub epoch: u64,
    /// Exact proposal inputs declared before any pinning or evaluation.
    pub proposal: crate::Proposal,
    /// Active accepted incumbent at preregistration.
    pub incumbent_id: String,
    /// Base host scoring policy; prediction-adjusted margin is derived later.
    pub policy: crate::ScoringPolicySpec,
    /// Hard per-run budget frozen before model execution.
    pub budget: crate::Budget,
    /// Immutable fingerprints for Screen, Selection, and Confirmation.
    pub suite_ids: std::collections::BTreeMap<String, String>,
}

/// Exact paired host outcomes retained separately from summary objectives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationEvent {
    /// Content identity of the evaluated candidate, possibly recorded later.
    pub candidate_id: String,
    /// Host stage, such as screening, selection, or confirmation.
    pub stage: String,
    /// Incumbent outcomes for the identical tasks and seeds.
    pub incumbent: Vec<crate::EvalRun>,
    /// Candidate outcomes for the identical tasks and seeds.
    pub candidate: Vec<crate::EvalRun>,
    /// Hard per-run cap frozen for this comparison; absent in legacy evidence.
    #[serde(default)]
    pub budget: Option<crate::Budget>,
    /// Exact statistical policy, including any prediction-adjusted margin.
    #[serde(default)]
    pub policy: Option<crate::EvalPolicy>,
    /// Host comparison recomputed and verified when the archive is replayed.
    #[serde(default)]
    pub comparison: Option<crate::Comparison>,
}

/// Exact normalized evidence tied to the incumbent, proposal, and frozen suites.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedEvaluationEvent {
    /// Prepared candidate content identity, including pinned memory.
    pub candidate_id: String,
    /// Accepted incumbent identity when the attempt started.
    pub incumbent_id: String,
    /// Host epoch of the original proposal declaration.
    pub epoch: u64,
    /// Exact proposal with a `normalized_h` prediction.
    pub proposal: crate::Proposal,
    /// Screen, Selection, or Confirmation.
    pub stage: String,
    /// Actual private suite fingerprint supplied by the host executor.
    pub suite_id: String,
    /// All three frozen suite identities, committed from the first stage.
    pub suite_ids: std::collections::BTreeMap<String, String>,
    /// Incumbent outcomes on the paired private draw.
    pub incumbent: Vec<crate::EvalRun>,
    /// Candidate outcomes on that same private draw.
    pub candidate: Vec<crate::EvalRun>,
    /// Measured incumbent health rates and reference cost ratio.
    pub incumbent_health: crate::ScoreHealth,
    /// Measured candidate health rates and relative token cost.
    pub candidate_health: crate::ScoreHealth,
    /// Shared per-run hard cap.
    pub budget: crate::Budget,
    /// Exact validated host policy including the prediction-adjusted margin.
    pub policy: crate::ScoringPolicySpec,
    /// Recomputed bootstrap evidence; never a converted raw-quality comparison.
    pub comparison: crate::ScoreComparison,
}

/// Human decision bound to exact confirmed normalized evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedApprovalEvent {
    /// Exact prepared design reviewed by the human.
    pub candidate_id: String,
    /// Exact confirmed normalized comparison supplied to the approval gate.
    pub comparison: crate::ScoreComparison,
    /// Human approval or denial; unavailable approval is not approval.
    pub approved: bool,
}

/// Candidate runs that failed where the incumbent's completed, kept as
/// evidence that the patch, not the host, broke the run.
///
/// Holds coarse counts and a failure kind only: no stderr, model reply or
/// private task text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateFailureEvent {
    /// Prepared or recorded content identity of the failed candidate.
    pub candidate_id: String,
    /// Host epoch of the attempt.
    pub epoch: u64,
    /// Screen, Selection, or Confirmation.
    pub stage: String,
    /// The first failure observed in job order.
    pub kind: crate::CandidateFailureKind,
    /// Candidate runs that failed; at least one.
    pub failed_runs: u32,
    /// Candidate runs the stage attempted; at least `failed_runs`.
    pub runs: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Event {
    Candidate {
        record: Box<Record>,
        package: Box<Package>,
    },
    Prepared {
        id: String,
        package: Box<Package>,
        watermarks: std::collections::BTreeMap<String, u64>,
    },
    Activation {
        id: String,
    },
    Proposal {
        proposal: ProposalEvent,
    },
    Declaration {
        declaration: ProposalDeclaration,
    },
    Evaluation {
        evaluation: EvaluationEvent,
    },
    NormalizedDeclaration {
        declaration: Box<NormalizedDeclarationEvent>,
    },
    NormalizedEvaluation {
        evaluation: Box<NormalizedEvaluationEvent>,
    },
    NormalizedApproval {
        approval: Box<NormalizedApprovalEvent>,
    },
    CandidateFailure {
        failure: CandidateFailureEvent,
    },
    Revocation {
        candidate_id: String,
        epoch: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snapshot {
    pub version: u64,
    pub events: Vec<Event>,
}
