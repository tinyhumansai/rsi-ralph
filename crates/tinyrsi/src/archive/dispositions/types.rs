//! Safe chronological host dispositions, independent of selection labels.
use crate::ReasonCode;
use serde::{Deserialize, Serialize};

/// Earlier exact-patch history available when the host stopped a duplicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuplicateDisposition {
    /// A replay-validated earlier language record accepted these operations.
    #[serde(rename = "DUP_ACCEPTED")]
    Accepted,
    /// Earlier records or explicit terminal stops rejected these operations.
    #[serde(rename = "DUP_REJECTED")]
    Rejected,
    /// No conclusive exact-patch history exists, including prepared content aliases.
    #[serde(rename = "DUP_UNRESOLVED")]
    Unresolved,
}
/// One safe projection of a durable proposal stop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalDispositionRow {
    /// Zero-based archive event index, distinguishing otherwise identical stops.
    pub event_index: usize,
    /// Original host epoch label.
    pub epoch: u64,
    /// Recognized coarse reason; unknown legacy text is never copied to output.
    pub reason: Option<ReasonCode>,
    /// Duplicate subtype, absent for nonduplicate or unrecognized reasons.
    pub duplicate: Option<DuplicateDisposition>,
}
/// Versioned read-only archive projection, containing no proposal payloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalDispositionReport {
    /// Report format, currently one; independent of archive/dataset versions.
    pub version: u32,
    /// Every durable proposal stop in chronological order.
    pub rows: Vec<ProposalDispositionRow>,
}
