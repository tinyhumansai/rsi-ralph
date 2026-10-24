//! Entry types held by the lineage ledger.
//!
//! Their serde form is pinned in `test.rs`, so a stored ledger written by one
//! version decodes in the next.

use serde::{Deserialize, Serialize};

/// What the loop decided about a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Verdict {
    /// The candidate's private grade beat the incumbent's; it is the new
    /// incumbent.
    Accepted,
    /// The candidate's private grade did not beat the incumbent's.
    Rejected,
}

/// One graded candidate in the improvement lineage.
///
/// A candidate is a snapshot of the agent's environment configuration, its
/// private grade across the selection benchmark, and the verdict the loop
/// reached about it. The grade is unitless and higher is better; the loop that
/// produces entries owns its scale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GradeEntry {
    /// The candidate's position in the lineage, zero-based and contiguous.
    pub step: u64,
    /// Content address of the candidate's environment snapshot.
    pub snapshot: String,
    /// The candidate's private grade across the selection benchmark.
    pub grade: f64,
    /// What the loop decided about this candidate.
    pub verdict: Verdict,
    /// Free-form rationale the loop attached to the decision.
    pub rationale: String,
}

impl GradeEntry {
    /// Builds an entry from its parts.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rsi_ralph::{GradeEntry, Verdict};
    /// let entry = GradeEntry::new(0, "sha256:abc", 0.703, Verdict::Accepted, "baseline");
    /// assert_eq!(entry.step, 0);
    /// assert_eq!(entry.snapshot, "sha256:abc");
    /// ```
    #[must_use]
    pub fn new(
        step: u64,
        snapshot: impl Into<String>,
        grade: f64,
        verdict: Verdict,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            step,
            snapshot: snapshot.into(),
            grade,
            verdict,
            rationale: rationale.into(),
        }
    }
}
