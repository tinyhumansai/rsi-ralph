//! Safe generation identities, selection labels and metric denominators.
use crate::{EvalStage, GenerationAudit, GenerationMode, ModelIdentity, ReasonCode};
use serde::{Deserialize, Serialize};
/// A formatter occurrence, unique within one epoch including identical items.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalOccurrence {
    /// One-based durable formatter call ID.
    pub call_id: u64,
    /// Zero-based item offset in the original returned array.
    pub item_index: usize,
}
/// Last observed host stage; no generated assertion can advance it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStage {
    /// Strictly decoded generation has not prepared a design.
    Generated,
    /// A candidate was prepared without completed checker evidence.
    Prepared,
    /// Screening evidence is retained.
    Screen,
    /// Selection evidence is retained.
    Selection,
    /// Independent confirmation ran; its measurements are excluded from this dataset.
    Confirmation,
}
impl From<EvalStage> for ProposalStage {
    fn from(stage: EvalStage) -> Self {
        match stage {
            EvalStage::Screen => Self::Screen,
            EvalStage::Selection => Self::Selection,
            EvalStage::Confirmation => Self::Confirmation,
        }
    }
}
/// Why a row does or does not supply a resolved selection label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalLabel {
    /// Selection measured acceptance eligibility under all frozen hard gates.
    Observed,
    /// Selection ran but its interval did not resolve eligibility.
    Inconclusive,
    /// The host has not yet attempted this proposal.
    Pending,
    /// A previous accepted proposal stopped the epoch before this occurrence.
    Censored,
    /// Duplicate history does not label this distinct occurrence.
    Duplicate,
    /// A preselection stop has no selection-stage outcome.
    NotSelected,
    /// Evaluation was interrupted before a selection result was durable.
    Interrupted,
}
/// Selection measurements only, excluding private draws and free-form reasons.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalSelection {
    /// Candidate minus incumbent normalized H on selection draws.
    pub delta: f64,
    /// Paired lower selection bound.
    pub lower: f64,
    /// Paired upper selection bound.
    pub upper: f64,
    /// Selection acceptance eligibility; absent for inconclusive evidence.
    pub eligible: Option<bool>,
}
/// One typed occurrence; raw proposal prose and patches are deliberately excluded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalOutcomeRow {
    /// Zero-based position in persisted evaluation order, retaining duplicates.
    pub proposal_index: usize,
    /// Original formatter occurrence, absent for legacy unmapped generation.
    pub occurrence: Option<ProposalOccurrence>,
    /// SHA-256 of the host formatter run identifier; absent for legacy generation.
    pub formatter_run_id: Option<String>,
    /// Whether generation ranking selected this occurrence.
    pub ranked: bool,
    /// Preregistered normalized-H minimum gain; never interpreted as probability.
    pub predicted_gain: f64,
    /// Host probability of selection eligibility, frozen before evaluation if supplied.
    pub prior: Option<f64>,
    /// Last stage established by retained actual host evidence.
    pub stage: ProposalStage,
    /// Durable host stopping reason; absent until its outcome is committed.
    pub reason: Option<ReasonCode>,
    /// Prepared content identity, absent before preparation or for an invalid edit.
    pub candidate_id: Option<String>,
    /// Explicit missing, unresolved or observed label classification.
    pub label: ProposalLabel,
    /// Actual selection-stage measurements; confirmation and anchors never enter it.
    pub selection: Option<ProposalSelection>,
    /// An unfinished host transaction remains visible even after a durable selection.
    pub interrupted: bool,
}
/// Versioned projection of a private journal and replay-validated archive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalOutcomeDataset {
    /// Public dataset format version, currently one.
    pub version: u32,
    /// Observed epoch identity.
    pub epoch: u64,
    /// Frozen input identity, also identifying this epoch's generation run.
    pub fingerprint: String,
    /// Experimental source arm.
    pub mode: GenerationMode,
    /// Public host model configuration, absent when unknown or legacy.
    pub model: Option<ModelIdentity>,
    /// Frozen generation seed, absent in legacy journals.
    pub generation_seed: Option<u64>,
    /// Redacted formatter ledger, including invalid items and failed/pending calls.
    pub generation: Option<GenerationAudit>,
    /// Aggregate generation accounting retained by the epoch transaction.
    /// It is a partial snapshot when `generation_complete` is false.
    pub meter: crate::GenerationMeter,
    /// Whether generation finished and committed its final accounting, including failures.
    pub generation_complete: bool,
    /// Every strictly decoded persisted proposal occurrence in evaluation order.
    pub rows: Vec<ProposalOutcomeRow>,
}
/// Descriptive selection-only metrics with explicit complete-case denominators.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalOutcomeMetrics {
    /// Typed occurrences, including unevaluated and duplicate items.
    pub rows: usize,
    /// Rows without a registered probability.
    pub missing_priors: usize,
    /// Rows without a resolved selection eligibility label.
    pub missing_labels: usize,
    /// Resolved labels paired with a registered probability.
    pub brier_pairs: usize,
    /// Mean squared probability error; absent without complete pairs.
    pub brier: Option<f64>,
    /// Rows with actual selection gain for tied-rank correlation.
    pub gain_pairs: usize,
    /// Tied-rank Spearman correlation; absent with fewer than two or constant ranks.
    pub spearman: Option<f64>,
    /// Requested top-k over all rows, including censored rows.
    pub k: usize,
    /// Resolved labels among the top-k rows in predicted-gain order.
    pub top_k_observed: usize,
    /// Eligible share, absent unless all top-k labels are resolved.
    pub precision_at_k: Option<f64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::epoch_driver) struct Origin {
    pub occurrence: ProposalOccurrence,
    pub run_id: String,
}
