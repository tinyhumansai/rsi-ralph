//! What a scorer reports and what the decision rule returns.
use tinyrsi_trace::{MetricDirection, ScoreDecision};

/// Raw per-seed metric values behind an assessment, and which way is better.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricEvidence {
    /// One value per seed, aligned with the assessed seeds.
    pub values: Vec<f64>,
    /// Whether higher or lower values are better.
    pub direction: MetricDirection,
}

/// One scorer's view of one subject.
#[derive(Debug, Clone, PartialEq)]
pub struct Assessment {
    /// One quality per seed in `[0, 1]`, higher is better, aligned with the
    /// assessed seeds.
    pub qualities: Vec<f64>,
    /// The qualities from evidence allowed to support acceptance on its own,
    /// or `None` when there is none (for example an uncalibrated judge).
    pub eligible: Option<Vec<f64>>,
    /// The raw metric behind the qualities, when the scorer measured one.
    pub metric: Option<MetricEvidence>,
    /// A judge's reasoning, when there is one.
    pub rationale: Option<String>,
    /// Measured model tokens spent producing this assessment.
    pub tokens: u64,
}

/// The decision for one candidate against the incumbent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decision {
    /// Accept, reject or inconclusive, after the eligibility gate.
    pub decision: ScoreDecision,
    /// Mean paired improvement, positive meaning better: in metric units when
    /// both sides carry a metric, otherwise in quality units.
    pub estimate: f64,
    /// Lower interval bound of the improvement, in the same units.
    pub lower: f64,
    /// Upper interval bound of the improvement, in the same units.
    pub upper: f64,
}
