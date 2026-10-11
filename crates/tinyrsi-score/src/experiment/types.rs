//! Frozen experiment contracts and redacted evidence summaries.

use crate::{Budget, EvalRun, ScoreComparison, ScoreHealth, ScoringPolicySpec};
use serde::{Deserialize, Serialize};

/// Provenance of actual callback measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceSource {
    /// Deterministic test fixtures, never evidence of model improvement.
    Scripted,
    /// Measurements obtained from a live model and host verifiers.
    Live,
}

/// Orchestrator configuration assigned before model output is observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExperimentArm {
    /// Frozen original orchestrator.
    Original,
    /// Frozen proposed evolved orchestrator.
    Evolved,
}

/// Preregistered intervention on the orchestrator's optional mechanisms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExperimentAblation {
    /// Memory and retreat enabled.
    Full,
    /// Memory disabled, retreat enabled.
    MemoryOff,
    /// Retreat disabled, memory enabled.
    RetreatOff,
}

/// Immutable host digests of three private, disjoint task suites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuiteFingerprints {
    /// Proposal selection suite digest.
    pub selection: String,
    /// Independent confirmation suite digest.
    pub confirmation: String,
    /// Untouched final anchor suite digest.
    pub anchor: String,
}

/// Independent generation and paired anchor evaluation seeds for one trial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrialSeeds {
    /// Generation seed, unique across independent trials.
    pub proposal_seed: u64,
    /// At least two anchor seeds, disjoint across independent trials.
    pub evaluation_seeds: Vec<u64>,
}

/// Serializable preregistration rebuilt through a validated constructor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentProtocolSpec {
    /// Original orchestrator artifact digest.
    pub original_config_id: String,
    /// Evolved orchestrator artifact digest.
    pub evolved_config_id: String,
    /// Identical starting solver hive digest for every trial.
    pub initial_hive_id: String,
    /// Hidden suite fingerprints fixed before callback execution.
    pub suites: SuiteFingerprints,
    /// Independent trial draws shared by both arms and all ablations.
    pub trials: Vec<TrialSeeds>,
    /// Proposal attempts per arm, ablation, and independent trial.
    pub attempts_per_trial: u32,
    /// Equal generation token cap per arm, ablation, and trial.
    pub generation_cap: u64,
    /// Equal token cap for all proposal and initial/final anchor evaluations.
    pub evaluation_cap: u64,
    /// Hard cap for each individual verifier run.
    pub per_run_budget: Budget,
    /// Fixed normalization, penalties, and bootstrap policy.
    pub scoring: ScoringPolicySpec,
    /// Required positive efficiency delta, in H units per 1,000 total tokens.
    pub efficiency_margin: f64,
    /// Provenance fixed before callback execution.
    pub source: EvidenceSource,
}

/// Validated immutable protocol and its SHA-256 preregistration digest.
#[derive(Debug, Clone)]
pub struct ExperimentProtocol {
    pub(super) spec: ExperimentProtocolSpec,
    pub(super) fingerprint: String,
}

/// Complete immutable callback assignment; it contains no task text or oracle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrialRequest {
    /// Preregistration digest binding every assignment.
    pub protocol_id: String,
    /// Assigned orchestrator arm.
    pub arm: ExperimentArm,
    /// Assigned memory/retreat intervention.
    pub ablation: ExperimentAblation,
    /// Paired generation and anchor evaluation draws.
    pub seeds: TrialSeeds,
    /// Assigned immutable orchestrator artifact digest.
    pub config_id: String,
    /// Identical starting hive artifact digest.
    pub initial_hive_id: String,
    /// Frozen hidden suite fingerprints.
    pub suites: SuiteFingerprints,
    /// Number of attempts, including rejected or failed attempts.
    pub attempts_per_trial: u32,
    /// Hard total generation cap.
    pub generation_cap: u64,
    /// Hard total evaluation cap, including anchors.
    pub evaluation_cap: u64,
    /// Cap on each individual verifier run.
    pub per_run_budget: Budget,
}

/// Outcome of an actual proposal attempt, including failed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttemptStatus {
    /// Confirmed and approved by the normalized proposal runner.
    Accepted,
    /// A proposal failed a hard or statistical gate.
    Rejected,
    /// Available evidence did not resolve the acceptance threshold.
    Inconclusive,
    /// Generation, execution, or the verifier failed.
    Failed,
}

/// Exact token spend of one attempt; failures still consume the trial caps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptEvidence {
    /// Actual normalized-runner disposition.
    pub status: AttemptStatus,
    /// All generation tokens, including retries and failed calls.
    pub generation_tokens: u64,
    /// All screening, selection, and confirmation evaluation tokens.
    pub evaluation_tokens: u64,
}

/// Host-private callback evidence; never embedded in the aggregate report.
#[derive(Debug, Clone, PartialEq)]
pub struct TrialEvidence {
    /// Exact assignment that was executed.
    pub request: TrialRequest,
    /// Every attempt, including all failures; length equals requested count.
    pub attempts: Vec<AttemptEvidence>,
    /// Starting hive outcomes on the private anchor matrix.
    pub initial_anchor: Vec<EvalRun>,
    /// Resulting hive outcomes on the identical private anchor matrix.
    pub final_anchor: Vec<EvalRun>,
    /// Starting hive telemetry, with cost ratio one.
    pub initial_health: ScoreHealth,
    /// Resulting hive telemetry, with cost ratio relative to its starting hive.
    pub final_health: ScoreHealth,
}

/// Typed reason an assignment could not contribute valid paired evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrialFailure {
    /// Callback failed; complete trial caps are conservatively charged.
    HostError,
    /// Callback returned malformed, mismatched, or unverifiable evidence.
    InvalidEvidence,
}

/// Public redacted trial ledger, with no task identities, prompts, or answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrialSummary {
    /// Assigned arm.
    pub arm: ExperimentArm,
    /// Assigned ablation.
    pub ablation: ExperimentAblation,
    /// Independent generation seed.
    pub proposal_seed: u64,
    /// Total attempts, including failures.
    pub attempts: u32,
    /// Confirmed and approved attempts.
    pub accepted: u32,
    /// Rejected attempts.
    pub rejected: u32,
    /// Statistically unresolved attempts.
    pub inconclusive: u32,
    /// Failed attempts.
    pub failed: u32,
    /// Generation spend, or the full reservation after a host failure.
    pub generation_tokens: u64,
    /// Evaluation spend including both anchors, or the full reservation.
    pub evaluation_tokens: u64,
    /// Whether the supplied usage and evidence passed validation.
    pub verified: bool,
    /// Optional failure classification without host messages or private text.
    pub failure: Option<TrialFailure>,
    /// Anchor H gain; absent for invalid or failed trials.
    pub anchor_gain: Option<f64>,
    /// Lower paired anchor gain bound.
    pub anchor_lower: Option<f64>,
    /// Upper paired anchor gain bound.
    pub anchor_upper: Option<f64>,
    /// Whether the paired anchor lower bound and every hard gate passed.
    pub anchor_accepted: bool,
}

/// Paired efficiency evidence for one preregistered ablation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AblationEstimate {
    /// Mechanism intervention being compared.
    pub ablation: ExperimentAblation,
    /// Complete original/evolved trial pairs.
    pub pairs: u32,
    /// Evolved minus original H gain per 1,000 total tokens.
    pub efficiency_delta: f64,
    /// Lower 95% percentile bound from resampling independent paired trials.
    pub lower: f64,
    /// Upper 95% percentile bound from resampling independent paired trials.
    pub upper: f64,
    /// Every evolved trial accepted a proposal and both anchor comparisons passed.
    pub anchors_accepted: bool,
}

/// Evidence conclusion under this bounded protocol; never an ignition claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExperimentVerdict {
    /// Consistent held-out, replicated improvement under the frozen protocol.
    SupportedImprovement,
    /// The complete full-mechanism interval excludes the required improvement.
    Negative,
    /// Failures, missing pairs, regression gates, or overlapping uncertainty.
    Inconclusive,
    /// Scripted measurements exercise the protocol and do not establish efficacy.
    ProtocolOnly,
}

/// Resampling method used for the experiment-level uncertainty interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExperimentUncertainty {
    /// Two-sided 95% percentile bootstrap of complete independent paired trials.
    PairedTrialPercentileBootstrap,
}

/// Serializable aggregate evidence with no private task data or model output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentReport {
    /// Scientific method for the reported experiment-level bounds.
    pub uncertainty: ExperimentUncertainty,
    /// Preregistered number of deterministic bootstrap resamples.
    pub bootstrap_resamples: u32,
    /// Required efficiency improvement in H units per 1,000 total tokens.
    pub efficiency_margin: f64,
    /// Identical generation cap for every assigned arm/ablation/trial.
    pub generation_cap_per_trial: u64,
    /// Identical evaluation cap, including anchors, for every assignment.
    pub evaluation_cap_per_trial: u64,
    /// Immutable preregistration SHA-256 digest.
    pub protocol_id: String,
    /// Declared measurement provenance.
    pub source: EvidenceSource,
    /// Counted attempts over both arms and all ablations.
    pub attempts: u64,
    /// Generation spend including conservative failure reservations.
    pub generation_tokens: u64,
    /// Evaluation spend including anchors and failure reservations.
    pub evaluation_tokens: u64,
    /// Redacted ledger for every assignment, including host failures.
    pub trials: Vec<TrialSummary>,
    /// Paired estimates for all three ablations.
    pub ablations: Vec<AblationEstimate>,
    /// Preregistered evidence conclusion.
    pub verdict: ExperimentVerdict,
}

/// Host executor of actual model generation and normalized proposal attempts.
pub trait ExperimentExecutor {
    /// Execute from a fresh copy of the frozen initial hive under hard caps.
    ///
    /// The host enforces caps before dispatch, excludes private anchors from
    /// proposal context, and derives all telemetry from actual model/verifier
    /// calls. Returning an error charges both complete reservations.
    ///
    /// # Errors
    /// Returns a host error when no complete trustworthy ledger is available.
    fn execute_trial(&mut self, request: &TrialRequest) -> crate::Result<TrialEvidence>;
}

/// Private retained trial evidence and its recomputed anchor comparison.
#[derive(Debug)]
pub(super) struct ValidTrial {
    pub evidence: TrialEvidence,
    pub summary: TrialSummary,
    pub comparison: ScoreComparison,
}
