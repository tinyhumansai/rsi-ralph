//! Scoring a measured metric.
//!
//! [`MetricScorer`] maps a per-seed metric to a quality with the monotone
//! transform `q = 1/2 + atan(s * m) / pi` (`s = +1` to maximize, `-1` to
//! minimize). It preserves order, so a better metric is always a higher
//! quality, and it lands strictly inside `(0, 1)`. A measured metric is
//! accept-eligible evidence, and it keeps its raw values so [`crate::decide`]
//! reports the improvement in the metric's own units.
use crate::port::orientation;
use crate::{Assessment, Error, MetricEvidence, Result, Scorer};
use rsi_ralph_trace::MetricDirection;
use rsi_ralph_types::Goal;
use std::f64::consts::PI;

/// Scores per-seed metric values; the subject is the slice of values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricScorer {
    direction: MetricDirection,
}

impl MetricScorer {
    /// Scores a metric where `direction` says which way is better.
    #[must_use]
    pub fn new(direction: MetricDirection) -> Self {
        Self { direction }
    }

    /// The quality of one metric value, in `(0, 1)`.
    #[must_use]
    pub fn quality(&self, metric: f64) -> f64 {
        0.5 + (orientation(self.direction) * metric).atan() / PI
    }

    /// True when `metric` meets `target` in this scorer's direction.
    #[must_use]
    pub fn reaches(&self, metric: f64, target: f64) -> bool {
        orientation(self.direction) * (metric - target) >= 0.0
    }

    /// True when `a` is strictly better than `b`.
    #[must_use]
    pub fn better(&self, a: f64, b: f64) -> bool {
        orientation(self.direction) * (a - b) > 0.0
    }
}

impl Scorer<[f64]> for MetricScorer {
    /// # Errors
    ///
    /// Returns [`Error::InvalidEvaluation`] when there is not exactly one
    /// finite value per seed.
    fn assess(&self, metrics: &[f64], _: &Goal, seeds: &[u64]) -> Result<Assessment> {
        if metrics.len() != seeds.len() || metrics.iter().any(|m| !m.is_finite()) {
            return Err(Error::InvalidEvaluation);
        }
        let qualities: Vec<f64> = metrics.iter().map(|m| self.quality(*m)).collect();
        Ok(Assessment {
            eligible: Some(qualities.clone()),
            qualities,
            metric: Some(MetricEvidence {
                values: metrics.to_vec(),
                direction: self.direction,
            }),
            rationale: None,
            tokens: 0,
        })
    }
}

#[cfg(test)]
mod test;
