//! Redacted formatter call identities and per-occurrence dispositions.
use super::GenerationMode;
use serde::{Deserialize, Serialize};
/// Host classification before ranking or private-suite evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationDisposition {
    /// The item failed the frozen public proposal schema.
    SchemaRejected,
    /// The schema-valid item failed strict typed decoding.
    DecodeRejected,
    /// The decoded item failed the host public-text guard.
    PrivacyRejected,
    /// The item decoded and passed the public-text guard; no quality claim.
    Decoded,
}
/// Durable outcome of one formatter call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationCallStatus {
    /// Intent persisted; no durable response is known. Never implicitly retried.
    Pending,
    /// An items array was returned, including an empty array.
    Returned,
    /// The response lacked an items array.
    Malformed,
    /// The backend failed, including parse, reservation or provider failures.
    Failed,
}
/// One call, preserving item occurrences in their original order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationCallReceipt {
    /// Consecutive one-based ID within this epoch. Together with the zero-based
    /// offset in `items`, uniquely identifies every occurrence, even duplicates.
    pub id: u64,
    /// Whether a durable result is known.
    pub status: GenerationCallStatus,
    /// Safe dispositions only; generated bodies and raw errors are never stored.
    pub items: Vec<GenerationDisposition>,
}
/// Versioned audit bound to the same frozen input as the private epoch journal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationAudit {
    /// Receipt format version, currently one.
    pub version: u32,
    /// Observed epoch identity.
    pub epoch: u64,
    /// Frozen generation input fingerprint.
    pub fingerprint: String,
    /// Experimental generation arm; identifies direct versus Engine formatting.
    pub mode: GenerationMode,
    /// All attempted formatter calls, including failures and pending intents.
    pub calls: Vec<GenerationCallReceipt>,
}
