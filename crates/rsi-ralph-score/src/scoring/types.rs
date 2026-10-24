//! Immutable scoring configuration and serialized score reports.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::EvalDecision;

/// Frozen baseline, reference, importance, and regression limit for one family.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FamilyAnchor {
    /// Stable task-family identity.
    pub family: String,
    /// Mean task quality of the frozen baseline in `[0,1]`.
    pub baseline: f64,
    /// Mean task quality of the reference in `[0,1]`, above the baseline.
    pub reference: f64,
    /// Positive importance weight applied before family IQM aggregation.
    pub weight: f64,
    /// Maximum allowed loss in normalized family score.
    pub regression_limit: f64,
}

/// Host-measured rates and token-cost ratio, with host-owned denominators.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreHealth {
    /// False-claim rate in `[0,1]`.
    pub false_claim_rate: f64,
    /// Protected-surface tamper rate in `[0,1]`.
    pub tamper_rate: f64,
    /// Cost relative to the incumbent; must be finite and nonnegative.
    pub cost_ratio: f64,
}

/// Nonnegative penalty coefficients frozen by the evaluation owner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScorePenalties {
    /// Coefficient applied to the false-claim rate.
    pub false_claims: f64,
    /// Coefficient applied to the tamper rate.
    pub tamper: f64,
    /// Coefficient applied only to cost increases above one.
    pub cost: f64,
}

/// Frozen scoring policy constructed through validated host configuration.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoringPolicy {
    pub(super) anchors: BTreeMap<String, FamilyAnchor>,
    pub(super) penalties: ScorePenalties,
    pub(super) margin: f64,
    pub(super) cost_tolerance: f64,
    pub(super) resamples: u32,
    pub(super) bootstrap_seed: u64,
}

/// Normalized hive score and its task-and-seed percentile-bootstrap interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreEstimate {
    /// Weighted family IQM minus health and cost penalties.
    pub value: f64,
    /// Lower endpoint of the two-sided 95% percentile-bootstrap interval.
    pub lower: f64,
    /// Upper endpoint of the two-sided 95% percentile-bootstrap interval.
    pub upper: f64,
    /// Normalized family means before importance weights and penalties.
    pub per_family: BTreeMap<String, f64>,
    /// Host telemetry used to compute the penalties.
    pub health: ScoreHealth,
}

/// Paired normalized-H difference with empirical hard regression gates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreComparison {
    /// Incumbent estimate under the frozen references.
    pub incumbent: ScoreEstimate,
    /// Candidate estimate under the same references and bootstrap draws.
    pub candidate: ScoreEstimate,
    /// Candidate H minus incumbent H.
    pub delta: f64,
    /// Lower percentile-bootstrap bound on the paired delta.
    pub lower: f64,
    /// Upper percentile-bootstrap bound on the paired delta.
    pub upper: f64,
    /// Acceptance eligibility, rejection, or need for more independent evidence.
    pub decision: EvalDecision,
}

/// Monitoring advice from consecutive anchor epochs; the host owns any rollback.
///
/// `rsi-ralph-host`'s `DeploymentStore::enforce_anchor` rolls back on
/// `regression` and treats `widening_gap` alone as advice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchorSignal {
    /// Consecutive anchor intervals fell below the best previous anchor.
    pub regression: bool,
    /// Consecutive selection-anchor gaps exceeded the initial gap.
    pub widening_gap: bool,
    /// Epoch of the highest observed anchor point estimate.
    pub best_epoch: u64,
}

/// In-memory monitoring history; it never changes an archive's active head.
#[derive(Debug, Clone)]
pub struct AnchorLedger {
    pub(super) required_epochs: usize,
    pub(super) regression_margin: f64,
    pub(super) gap_margin: f64,
    pub(super) best: Option<(u64, ScoreEstimate)>,
    pub(super) initial_gap: Option<f64>,
    pub(super) last_epoch: Option<u64>,
    pub(super) regression_streak: usize,
    pub(super) gap_streak: usize,
}

/// Serializable trusted-host specification rebuilt through policy validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoringPolicySpec {
    /// Frozen reference anchors, one per task family.
    pub anchors: Vec<FamilyAnchor>,
    /// Fixed health and cost penalty coefficients.
    pub penalties: ScorePenalties,
    /// Required positive lower-bound normalized gain.
    pub margin: f64,
    /// Fractional allowed token-cost increase.
    pub cost_tolerance: f64,
    /// Number of task-and-seed bootstrap draws, at least 200.
    pub resamples: u32,
    /// Deterministic bootstrap stream seed.
    pub bootstrap_seed: u64,
}
