//! Bounds and host-labeled outcomes for retreat history.
use serde::{Deserialize, Serialize};
/// Independent hard limits for discovery and host-outcome rolling windows.
#[derive(Clone, Debug)]
pub struct MemoryLimits {
    /// Maximum entries retained in each window; oldest entries are evicted.
    pub max_entries: usize,
    /// Maximum bytes in each window: UTF-8 discovery text or serialized outcomes.
    /// An individual entry exceeding this bound is rejected before eviction.
    pub max_bytes: usize,
}
impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            max_bytes: 1024 * 1024,
        }
    }
}
/// Host evaluation label, separate from creative-engine selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetreatOutcomeStatus {
    /// Host accepted and approved the evaluated candidate.
    Approved,
    /// Host rejected the candidate.
    Rejected,
}
/// Redacted host outcome; never contains private patches or oracle output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetreatMemoryOutcome {
    /// Host epoch.
    pub epoch: u64,
    /// Public content identity.
    pub candidate_id: String,
    /// Host outcome classification.
    pub status: RetreatOutcomeStatus,
}
