//! Durable generation modes, limits and redacted epoch receipts.
use crate::{GenerationMeter, Proposal, ReasonCode};
use serde::{Deserialize, Serialize};
/// Experimental generation arm; all arms retain the same proposal schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationMode {
    /// Real critic-pair Engine with durable discovery memory.
    Retreat,
    /// Real critic-pair Engine with recall and storage disabled.
    RetreatWithoutMemory,
    /// One direct native-tool proposer, without critics, ranking or memory.
    Direct,
}
/// Frozen generation caps shared by experimental arms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationBudget {
    /// Whole-generation reservation cap, including failed calls.
    pub total_tokens: u64,
    /// Maximum reservation for one model call.
    pub call_tokens: u64,
    /// Provider-enforced output tokens per call.
    pub output_tokens: u64,
    /// Model-call timeout in milliseconds.
    pub timeout_ms: u64,
    /// Independent conversations.
    pub variations: u32,
    /// Conversation turns per variation.
    pub generations: u32,
    /// Extracted items per variation.
    pub items_per_variation: u32,
    /// Maximum ranked proposals returned in the one generation.
    pub select_top_k: u32,
    /// Independent judge ratings per proposal.
    pub judge_raters: u32,
    /// Whole Engine wall-clock limit in seconds.
    pub wall_clock_secs: u64,
}
/// Durable strictly decoded generation, ready for synchronous host evaluation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedRetreatEpoch {
    /// Exactly-once observed epoch identity.
    pub epoch: u64,
    /// Frozen incumbent content identity.
    pub incumbent_id: String,
    /// Experimental arm.
    pub mode: GenerationMode,
    /// Frozen input and policy fingerprint.
    pub fingerprint: String,
    /// Frozen evaluation-policy and private-suite fingerprint.
    pub policy_fingerprint: String,
    /// Strict proposals in returned ranking order.
    /// Ranked selections precede schema-valid formatter occurrences not selected.
    pub proposals: Vec<Proposal>,
    /// Strict proposals selected by the Engine ranking (all proposals for direct mode).
    pub ranked_items: usize,
    /// Returned items rejected by strict decoding or privacy checks.
    pub invalid_items: usize,
    /// Redacted accounting, including failed and refused model calls.
    pub meter: GenerationMeter,
}
/// Durable host evaluation receipt, without private task or model payloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpochCandidateOutcome {
    /// Zero-based generated proposal index.
    pub proposal_index: usize,
    /// Host evaluation classification.
    pub reason: ReasonCode,
    /// Prepared content identity, absent when formalization failed.
    pub candidate_id: Option<String>,
}
/// Redacted completion receipt; activation is a separate host transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetreatEpochOutcome {
    /// Observed epoch identity.
    pub epoch: u64,
    /// Host classifications in generation ranking order.
    pub candidates: Vec<EpochCandidateOutcome>,
    /// True once evaluation ended or accepted a candidate.
    pub complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    pub epoch: u64,
    pub incumbent_id: String,
    pub fingerprint: String,
    pub prepared: Option<PreparedRetreatEpoch>,
    pub meter: GenerationMeter,
    pub generation_failed: bool,
    #[serde(default)]
    pub generation_receipts: bool,
    #[serde(default)]
    pub generation_mode: Option<GenerationMode>,
    pub invalid_items: usize,
    pub evaluating: bool,
    pub outcome: RetreatEpochOutcome,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snapshot {
    pub version: u32,
    pub entries: std::collections::BTreeMap<u64, Entry>,
}

impl Entry {
    pub(super) fn intent(
        epoch: u64,
        incumbent_id: String,
        fingerprint: String,
        meter: GenerationMeter,
        mode: GenerationMode,
    ) -> Self {
        Self {
            epoch,
            incumbent_id,
            fingerprint,
            prepared: None,
            meter,
            generation_failed: false,
            generation_receipts: true,
            generation_mode: Some(mode),
            invalid_items: 0,
            evaluating: false,
            outcome: RetreatEpochOutcome {
                epoch,
                candidates: vec![],
                complete: false,
            },
        }
    }
}
