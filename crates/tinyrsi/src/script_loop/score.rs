//! Scoring candidates through the scoring port.
//!
//! The loop hands every fully measured candidate to a
//! `tinyrsi_score::Scorer<ScriptCandidate>` and lets `tinyrsi_score::decide`
//! compare it with the incumbent's cached assessment. The default scorer reads
//! only the per-seed metrics through `MetricScorer`, which reproduces the
//! loop's original decisions and metric-unit intervals exactly. A candidate
//! with a failed seed is never scored; it is rejected, and its interval is
//! reported over the seeds that did produce a metric.
use super::ScriptCandidate;
use crate::MetricDirection;
use tinyrsi_score::{MetricScorer, Projected, Scorer};

/// The scorer `run_script_loop` uses: the metric alone.
pub(super) fn default_scorer(direction: MetricDirection) -> impl Scorer<ScriptCandidate> {
    Projected::new(
        |c: &ScriptCandidate| c.metrics.as_slice(),
        MetricScorer::new(direction),
    )
}

/// Mean and standard-error interval of the oriented paired differences, for a
/// candidate that failed on some seed.
///
/// Pairs with a missing side are skipped; no pair at all is a zero interval.
pub(super) fn failed_interval(
    incumbent: &[Option<f64>],
    candidate: &[Option<f64>],
    direction: MetricDirection,
    multiplier: f64,
) -> (f64, f64, f64) {
    let sign = match direction {
        MetricDirection::Maximize => 1.0,
        MetricDirection::Minimize => -1.0,
    };
    let diffs: Vec<f64> = incumbent
        .iter()
        .zip(candidate)
        .filter_map(|(b, c)| Some(sign * (c.as_ref()? - b.as_ref()?)))
        .collect();
    let n = diffs.iter().fold(0.0, |n, _| n + 1.0);
    if diffs.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mean = diffs.iter().sum::<f64>() / n;
    if diffs.len() < 2 {
        return (mean, mean, mean);
    }
    let variance = diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let half = multiplier * (variance / n).sqrt();
    (mean, mean - half, mean + half)
}
