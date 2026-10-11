//! Coarse candidate failure evidence.
use serde::{Deserialize, Serialize};

/// Which way a candidate's run failed where the incumbent's completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateFailureKind {
    /// The run errored or produced no verifiable result.
    Crash,
    /// The model call outlived the host's timeout.
    Timeout,
    /// A request or reply did not fit its token reservation.
    OverBudget,
    /// The candidate lowered to a hive the evaluator cannot run.
    Unsupported,
}

/// Candidate runs that failed on draws the incumbent completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateFailure {
    /// The first failure observed, in job order.
    pub kind: CandidateFailureKind,
    /// Candidate runs that failed; at least one.
    pub failed_runs: u32,
    /// Candidate runs the draw attempted; at least `failed_runs`.
    pub runs: u32,
}
