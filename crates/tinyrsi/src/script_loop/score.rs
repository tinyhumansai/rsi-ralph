//! Paired per-seed scoring of a candidate against the incumbent.
//!
//! The decision is [`crate::compare`]'s, on qualities produced by the monotone
//! map `q = 1/2 + atan(s * m) / pi` (`s = +1` to maximize, `-1` to minimize).
//! It preserves order, so a better metric is always a higher quality, and lands
//! strictly inside `(0, 1)` as `compare` requires. The reported interval is in
//! metric units so a reader can see the improvement in the script's own terms.
use crate::{
    Budget, EvalDecision, EvalPolicy, EvalRun, MetricDirection, Result, ScoreDecision, compare,
};
use std::f64::consts::PI;

/// Decision and metric-unit improvement interval of one candidate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Scored {
    pub decision: ScoreDecision,
    pub estimate: f64,
    pub lower: f64,
    pub upper: f64,
}

/// `+1` when higher is better, `-1` when lower is better.
fn orientation(direction: MetricDirection) -> f64 {
    match direction {
        MetricDirection::Maximize => 1.0,
        MetricDirection::Minimize => -1.0,
    }
}

/// Maps a metric to a quality in `(0, 1)` that grows as the metric improves.
pub(super) fn quality(metric: f64, direction: MetricDirection) -> f64 {
    0.5 + (orientation(direction) * metric).atan() / PI
}

/// True when `metric` meets `target` in the given direction.
pub(super) fn reaches(metric: f64, target: f64, direction: MetricDirection) -> bool {
    orientation(direction) * (metric - target) >= 0.0
}

/// True when `a` is strictly better than `b`.
pub(super) fn better(a: f64, b: f64, direction: MetricDirection) -> bool {
    orientation(direction) * (a - b) > 0.0
}

/// Mean and standard-error interval of the oriented paired differences.
///
/// Pairs with a missing side are skipped; no pair at all is a zero interval.
pub(super) fn interval(
    incumbent: &[Option<f64>],
    candidate: &[Option<f64>],
    direction: MetricDirection,
    multiplier: f64,
) -> (f64, f64, f64) {
    let diffs: Vec<f64> = incumbent
        .iter()
        .zip(candidate)
        .filter_map(|(b, c)| Some(orientation(direction) * (c.as_ref()? - b.as_ref()?)))
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

/// Scores complete per-seed metrics of a candidate against the incumbent.
///
/// `seeds`, `incumbent` and `candidate` are aligned by index.
///
/// # Errors
///
/// Returns [`crate::Error::InvalidEvaluation`] when `compare` rejects the
/// policy or the matrix (for example fewer than two seeds).
pub(super) fn score(
    seeds: &[u64],
    incumbent: &[f64],
    candidate: &[f64],
    direction: MetricDirection,
    policy: &EvalPolicy,
) -> Result<Scored> {
    let runs = |metrics: &[f64]| -> Vec<EvalRun> {
        seeds
            .iter()
            .zip(metrics)
            .map(|(seed, metric)| EvalRun {
                task: "script".into(),
                family: "script".into(),
                seed: *seed,
                quality: quality(*metric, direction),
                tokens: 0,
                false_claims: 0,
                tamper: 0,
            })
            .collect()
    };
    let comparison = compare(
        &runs(incumbent),
        &runs(candidate),
        &Budget { max_tokens: 1 },
        policy,
    )?;
    let wrap = |m: &[f64]| m.iter().copied().map(Some).collect::<Vec<_>>();
    let (estimate, lower, upper) = interval(
        &wrap(incumbent),
        &wrap(candidate),
        direction,
        policy.standard_error_multiplier,
    );
    Ok(Scored {
        decision: match comparison.decision {
            EvalDecision::Accept { .. } => ScoreDecision::Accept,
            EvalDecision::Reject { .. } => ScoreDecision::Reject,
            EvalDecision::Inconclusive { .. } => ScoreDecision::Inconclusive,
        },
        estimate,
        lower,
        upper,
    })
}
