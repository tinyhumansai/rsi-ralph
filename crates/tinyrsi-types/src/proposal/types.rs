//! Proposal data: what a proposer submits and why a host stopped it.
use serde::{Deserialize, Serialize};
use tinyhivemind_lang::patch::Patch;

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

/// An attempted proposal that did not produce an evaluated language record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalEvent {
    /// Host epoch that attempted this proposal.
    pub epoch: u64,
    /// Exact attempted patch, including invalid operations.
    pub patch: Patch,
    /// Proposer's explanation.
    pub rationale: String,
    /// Host reason for rejecting or guarding the attempt.
    pub reason: String,
    /// Public transcript sequence references supporting the attempt.
    pub transcript_refs: Vec<u64>,
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
