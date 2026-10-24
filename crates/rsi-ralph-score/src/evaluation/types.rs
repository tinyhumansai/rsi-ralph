//! Serializable evaluation evidence and acceptance policy.

use serde::{Deserialize, Serialize};

/// Hard execution cap shared by every incumbent and candidate run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    /// Maximum tokens consumed by one run; must be positive.
    pub max_tokens: u64,
}

/// Host-supplied outcome of one held-out task and random seed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalRun {
    /// Stable private-task identity, without task contents.
    pub task: String,
    /// Task family used for equal-family aggregation.
    pub family: String,
    /// Reproducible execution seed.
    pub seed: u64,
    /// Finite verifier quality in the inclusive range zero to one.
    pub quality: f64,
    /// Tokens consumed under the shared cap.
    pub tokens: u64,
    /// Number of false claims observed by the host verifier.
    pub false_claims: u64,
    /// Number of tampering attempts observed by the host verifier.
    pub tamper: u64,
}

/// Explicit practical significance, uncertainty, and regression limits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalPolicy {
    /// Required positive lower-bound gain.
    pub margin: f64,
    /// Number of paired standard errors used for the interval (at least two).
    pub standard_error_multiplier: f64,
    /// Largest allowed mean quality loss in any family.
    pub family_regression_limit: f64,
    /// Allowed fractional token increase relative to the incumbent.
    pub cost_tolerance: f64,
}

impl Default for EvalPolicy {
    fn default() -> Self {
        Self {
            margin: 0.01,
            standard_error_multiplier: 2.0,
            family_regression_limit: 0.0,
            cost_tolerance: 0.0,
        }
    }
}

/// Screening or full-evaluation disposition; acceptance still needs confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvalDecision {
    /// The measured lower bound beats the margin and every constraint passes.
    Accept {
        /// Explanation retained as evaluation evidence.
        reason: String,
    },
    /// A constraint failed or even the upper bound cannot beat the margin.
    Reject {
        /// Explanation retained as evaluation evidence.
        reason: String,
    },
    /// The interval overlaps the acceptance threshold.
    Inconclusive {
        /// Explanation retained as evaluation evidence.
        reason: String,
    },
}

/// Paired, equal-family comparison of two complete evaluation matrices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// Candidate quality minus incumbent quality, equally weighting families.
    pub gain: f64,
    /// Standard error across independent seed-level paired means.
    pub standard_error: f64,
    /// Gain minus the configured multiple of standard error.
    pub lower: f64,
    /// Gain plus the configured multiple of standard error.
    pub upper: f64,
    /// Acceptance, clear rejection, or insufficient evidence.
    pub decision: EvalDecision,
}

/// Human-label calibration of one binary rubric or preference judge.
///
/// Agreement is Cohen's kappa, computed from paired private gold labels rather
/// than a judge's self-reported reliability. The host owns these labels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgeCalibration {
    pub(super) agreement: f64,
    pub(super) threshold: f64,
}

/// Strength of an eval-owned checkpoint verifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerifierTier {
    /// A proof checked by a kernel; fixed weight one.
    Proof,
    /// Executed tests or golden outputs; fixed weight 0.9.
    Execution,
    /// Schema or property verification; fixed weight 0.7.
    Schema,
    /// A private held-out reference; fixed weight 0.7.
    HeldOutReference,
    /// Rubric judgment weighted by measured human agreement.
    RubricJudge(JudgeCalibration),
    /// Preference judgment discounted to half its measured human agreement.
    Preference(JudgeCalibration),
}

/// Frozen eval-owned checkpoint tiers and goal weight for one task.
///
/// The hive's blueprint nodes never supply checkpoints or weights. Construction
/// and deserialization belong to the trusted host's evaluation configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityRubric {
    pub(super) alpha: f64,
    pub(super) checkpoints: Vec<VerifierTier>,
}
