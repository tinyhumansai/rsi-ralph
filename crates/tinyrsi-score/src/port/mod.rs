//! The scoring port and the one rule that decides between two assessments.
//!
//! A [`Scorer`] turns a subject into an [`Assessment`]: one quality per seed.
//! It never decides. [`decide`] compares the incumbent's assessment with the
//! candidate's using [`crate::compare`], and applies two more rules:
//!
//! - **Eligibility.** An `Accept` stands only when `compare` also accepts on
//!   both sides' [`Assessment::eligible`] qualities. Without eligible evidence
//!   on both sides the result is capped at `Inconclusive`, so an uncalibrated
//!   judge can widen a margin but never decide one.
//! - **Units.** When both sides carry [`MetricEvidence`], the reported
//!   interval is the mean paired metric improvement plus or minus the policy's
//!   standard-error multiple, in the metric's own units. Otherwise it is
//!   `compare`'s interval in quality units.
mod types;

use crate::{Budget, EvalDecision, EvalPolicy, EvalRun, Result, compare};
use tinyrsi_trace::{MetricDirection, ScoreDecision};
use tinyrsi_types::Goal;
pub use types::{Assessment, Decision, MetricEvidence};

/// Turns a subject into per-seed qualities against a goal.
///
/// Implementations are synchronous; a model-backed scorer drives its future
/// on the calling thread (see `tinyrsi_model::block_on`).
pub trait Scorer<S: ?Sized> {
    /// Assesses `subject` once per seed in `seeds`.
    ///
    /// # Errors
    ///
    /// Returns an error when the subject cannot be assessed, for example a
    /// failed model call, a malformed judge reply, or per-seed input that does
    /// not match `seeds`.
    fn assess(&self, subject: &S, goal: &Goal, seeds: &[u64]) -> Result<Assessment>;
}

impl<S: ?Sized, T: Scorer<S> + ?Sized> Scorer<S> for Box<T> {
    fn assess(&self, subject: &S, goal: &Goal, seeds: &[u64]) -> Result<Assessment> {
        (**self).assess(subject, goal, seeds)
    }
}

/// `+1` when higher is better, `-1` when lower is better.
pub(crate) fn orientation(direction: MetricDirection) -> f64 {
    match direction {
        MetricDirection::Maximize => 1.0,
        MetricDirection::Minimize => -1.0,
    }
}

/// Mean and standard-error interval of the oriented paired differences, or
/// `None` when any part overflows to a non-finite value.
fn metric_interval(
    incumbent: &[f64],
    candidate: &[f64],
    direction: MetricDirection,
    multiplier: f64,
) -> Option<(f64, f64, f64)> {
    let diffs: Vec<f64> = incumbent
        .iter()
        .zip(candidate)
        .map(|(b, c)| orientation(direction) * (c - b))
        .collect();
    let n = diffs.iter().fold(0.0, |n, _| n + 1.0);
    let mean = diffs.iter().sum::<f64>() / n;
    let variance = diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let half = multiplier * (variance / n).sqrt();
    let interval = (mean, mean - half, mean + half);
    [interval.0, interval.1, interval.2]
        .iter()
        .all(|v| v.is_finite())
        .then_some(interval)
}

/// Eligible evidence must be as well-formed as the qualities it gates,
/// whatever the primary verdict turns out to be.
fn check_eligible(assessment: &Assessment, seeds: &[u64]) -> Result<()> {
    match &assessment.eligible {
        Some(e) if e.len() != seeds.len() || e.iter().any(|q| !(0.0..=1.0).contains(q)) => {
            Err(crate::Error::InvalidEvaluation)
        }
        _ => Ok(()),
    }
}

fn runs(seeds: &[u64], qualities: &[f64]) -> Vec<EvalRun> {
    seeds
        .iter()
        .zip(qualities)
        .map(|(seed, quality)| EvalRun {
            task: "subject".into(),
            family: "subject".into(),
            seed: *seed,
            quality: *quality,
            tokens: 0,
            false_claims: 0,
            tamper: 0,
        })
        .collect()
}

fn paired(
    seeds: &[u64],
    incumbent: &[f64],
    candidate: &[f64],
    policy: &EvalPolicy,
) -> Result<crate::Comparison> {
    if incumbent.len() != seeds.len() || candidate.len() != seeds.len() {
        return Err(crate::Error::InvalidEvaluation);
    }
    compare(
        &runs(seeds, incumbent),
        &runs(seeds, candidate),
        &Budget { max_tokens: 1 },
        policy,
    )
}

/// Decides whether `candidate` beats `incumbent` on the same `seeds`.
///
/// # Errors
///
/// Returns [`crate::Error::InvalidEvaluation`] when an assessment does not
/// have one quality (or eligible quality) per seed, a quality lies outside
/// `[0, 1]`, there are fewer than two distinct seeds, the metric interval
/// overflows, or `compare` rejects the policy.
///
/// # Examples
///
/// ```
/// use tinyrsi_score::{Assessment, EvalPolicy, ScoreDecision, decide};
///
/// let at = |q: f64| Assessment {
///     qualities: vec![q, q + 0.01, q - 0.01],
///     eligible: Some(vec![q, q + 0.01, q - 0.01]),
///     metric: None,
///     rationale: None,
///     tokens: 0,
/// };
/// let policy = EvalPolicy {
///     margin: 0.0,
///     standard_error_multiplier: 2.0,
///     family_regression_limit: 1.0,
///     cost_tolerance: 0.0,
/// };
/// let decision = decide(&at(0.2), &at(0.8), &[1, 2, 3], &policy)?;
/// assert_eq!(decision.decision, ScoreDecision::Accept);
/// # Ok::<(), tinyrsi_score::Error>(())
/// ```
pub fn decide(
    incumbent: &Assessment,
    candidate: &Assessment,
    seeds: &[u64],
    policy: &EvalPolicy,
) -> Result<Decision> {
    check_eligible(incumbent, seeds)?;
    check_eligible(candidate, seeds)?;
    let comparison = paired(seeds, &incumbent.qualities, &candidate.qualities, policy)?;
    let mut decision = match comparison.decision {
        EvalDecision::Accept { .. } => ScoreDecision::Accept,
        EvalDecision::Reject { .. } => ScoreDecision::Reject,
        EvalDecision::Inconclusive { .. } => ScoreDecision::Inconclusive,
    };
    if decision == ScoreDecision::Accept {
        let gated = match (&incumbent.eligible, &candidate.eligible) {
            (Some(i), Some(c)) => matches!(
                paired(seeds, i, c, policy)?.decision,
                EvalDecision::Accept { .. }
            ),
            _ => false,
        };
        if !gated {
            decision = ScoreDecision::Inconclusive;
        }
    }
    let (estimate, lower, upper) = match (&incumbent.metric, &candidate.metric) {
        (Some(i), Some(c))
            if i.direction == c.direction
                && i.values.len() == seeds.len()
                && c.values.len() == seeds.len() =>
        {
            metric_interval(
                &i.values,
                &c.values,
                c.direction,
                policy.standard_error_multiplier,
            )
            .ok_or(crate::Error::InvalidEvaluation)?
        }
        _ => (comparison.gain, comparison.lower, comparison.upper),
    };
    Ok(Decision {
        decision,
        estimate,
        lower,
        upper,
    })
}

#[cfg(test)]
pub(crate) mod test;
