//! Exact paired sign test over per-item scores.
//!
//! Benchmarks that grade each question pass or fail give one 0/1 score per
//! item for the incumbent and the candidate. Only discordant items, where the
//! two disagree, carry evidence. [`sign_test`] accepts when the candidate wins
//! more discordant items than it loses and an exact one-sided binomial test
//! rejects "no difference" at `alpha`, rejects on the mirror-image condition,
//! and is otherwise inconclusive. The reported estimate is the mean paired
//! difference with a 1.645-standard-error interval, in score units.

use std::collections::BTreeMap;

use crate::{Error, Result, ScoreDecision};

mod types;
pub use types::SignTest;

/// Normal quantile for a two-sided 90% interval around the estimate.
const Z: f64 = 1.645;

/// One-sided exact binomial tail `P(X >= wins)` for `X ~ Bin(wins + losses, 1/2)`.
///
/// Returns 1 when there are no discordant pairs. Computed in log space so it
/// stays finite for thousands of items.
#[must_use]
pub fn sign_test_p(wins: u64, losses: u64) -> f64 {
    let n = wins + losses;
    if n == 0 {
        return 1.0;
    }
    #[expect(clippy::cast_precision_loss, reason = "item counts are far below 2^52")]
    let nf = n as f64;
    // log pmf(k) by the recurrence pmf(k+1) = pmf(k) * (n-k)/(k+1).
    let mut log_pmf = -nf * std::f64::consts::LN_2;
    let mut terms = Vec::new();
    for k in 0..=n {
        if k >= wins {
            terms.push(log_pmf);
        }
        if k < n {
            #[expect(clippy::cast_precision_loss, reason = "item counts are far below 2^52")]
            let step = ((n - k) as f64).ln() - ((k + 1) as f64).ln();
            log_pmf += step;
        }
    }
    let peak = terms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let total: f64 = terms.iter().map(|t| (t - peak).exp()).sum();
    (peak + total.ln()).exp().min(1.0)
}

/// Compare two `item -> score` maps over the items they share.
///
/// # Errors
/// Returns [`Error::InvalidEvaluation`] when `alpha` is not inside `(0, 1)` or
/// any shared score is not finite.
pub fn sign_test(
    incumbent: &BTreeMap<String, f64>,
    candidate: &BTreeMap<String, f64>,
    alpha: f64,
) -> Result<SignTest> {
    if !(alpha > 0.0 && alpha < 1.0) {
        return Err(Error::InvalidEvaluation);
    }
    let mut diffs = Vec::new();
    for (item, before) in incumbent {
        if let Some(after) = candidate.get(item) {
            if !before.is_finite() || !after.is_finite() {
                return Err(Error::InvalidEvaluation);
            }
            diffs.push(after - before);
        }
    }
    if diffs.is_empty() {
        return Ok(SignTest {
            decision: ScoreDecision::Inconclusive,
            items: 0,
            wins: 0,
            losses: 0,
            p: 1.0,
            estimate: 0.0,
            lower: 0.0,
            upper: 0.0,
        });
    }
    let wins = diffs.iter().filter(|d| **d > 0.0).count() as u64;
    let losses = diffs.iter().filter(|d| **d < 0.0).count() as u64;
    #[expect(clippy::cast_precision_loss, reason = "item counts are far below 2^52")]
    let n = diffs.len() as f64;
    let mean = diffs.iter().sum::<f64>() / n;
    let var = diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    let half = Z * (var / n).sqrt();
    let p = sign_test_p(wins, losses);
    let decision = if wins > losses && p < alpha {
        ScoreDecision::Accept
    } else if losses > wins && sign_test_p(losses, wins) < alpha {
        ScoreDecision::Reject
    } else {
        ScoreDecision::Inconclusive
    };
    Ok(SignTest {
        decision,
        items: diffs.len(),
        wins,
        losses,
        p,
        estimate: mean,
        lower: mean - half,
        upper: mean + half,
    })
}

#[cfg(test)]
mod test;
