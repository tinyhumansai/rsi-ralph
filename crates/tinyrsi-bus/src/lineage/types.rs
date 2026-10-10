//! Payload types for the `lineage` member family.
//!
//! These types are the wire form: a host and a module that disagree about a
//! field name fail at runtime with a decode error, so the shapes are asserted
//! in `test.rs` rather than assumed.

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
    /// # use tinyrsi_bus::{GradeEntry, Verdict};
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

/// The argument to [`crate::names::methods::APPEND`].
///
/// The module rejects an entry whose [`GradeEntry::step`] does not exactly
/// continue the ledger, and one whose snapshot address or rationale is empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LineageAppendRequest {
    /// The graded candidate to record.
    pub entry: GradeEntry,
}

impl LineageAppendRequest {
    /// Builds a request recording `entry`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tinyrsi_bus::{GradeEntry, LineageAppendRequest, Verdict};
    /// let entry = GradeEntry::new(0, "sha256:abc", 0.5, Verdict::Accepted, "baseline");
    /// assert_eq!(LineageAppendRequest::new(entry).entry.step, 0);
    /// ```
    #[must_use]
    pub fn new(entry: GradeEntry) -> Self {
        Self { entry }
    }
}

/// The reply from [`crate::names::methods::APPEND`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LineageAppendResponse {
    /// True when the entry was recorded; false when the step already existed.
    pub recorded: bool,
}

impl LineageAppendResponse {
    /// Builds a reply carrying `recorded`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tinyrsi_bus::LineageAppendResponse;
    /// assert_eq!(LineageAppendResponse::new(true).recorded, true);
    /// ```
    #[must_use]
    pub fn new(recorded: bool) -> Self {
        Self { recorded }
    }
}

/// The argument to [`crate::names::methods::HEAD`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LineageHeadRequest {}

impl LineageHeadRequest {
    /// Builds a request for the current incumbent.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tinyrsi_bus::LineageHeadRequest;
    /// let request = LineageHeadRequest::new();
    /// assert!(request == LineageHeadRequest::default());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

/// The reply from [`crate::names::methods::HEAD`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LineageHeadResponse {
    /// The current incumbent, or `None` when the ledger is empty.
    pub entry: Option<GradeEntry>,
}

impl LineageHeadResponse {
    /// Builds a reply carrying `entry`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tinyrsi_bus::LineageHeadResponse;
    /// let reply = LineageHeadResponse::new(None);
    /// assert!(reply.entry.is_none());
    /// ```
    #[must_use]
    pub fn new(entry: Option<GradeEntry>) -> Self {
        Self { entry }
    }
}
