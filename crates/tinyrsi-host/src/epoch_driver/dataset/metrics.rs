//! Selection-only probability, tied-rank and top-k descriptive measurements.
use super::{
    ProposalLabel, ProposalOutcomeDataset, ProposalOutcomeMetrics, digest, model_valid, probability,
};
use crate::{Error, Result};
use std::cmp::Ordering;
impl ProposalOutcomeDataset {
    /// Computes selection-only metrics without imputing missing or censored labels.
    /// Precision ranks all rows by predicted gain, with proposal-index tie breaks.
    /// # Errors
    /// Rejects malformed datasets, nonfinite values, invalid probabilities, zero
    /// or excessive k, and datasets too large for exact count conversion.
    pub fn metrics(&self, k: usize) -> Result<ProposalOutcomeMetrics> {
        validate(self, k)?;
        let mut squared_error = 0.0;
        let mut brier_pairs = 0;
        let mut gains = Vec::new();
        for row in &self.rows {
            if let (Some(prior), Some(eligible)) =
                (row.prior, row.selection.as_ref().and_then(|s| s.eligible))
            {
                squared_error += (prior - f64::from(u8::from(eligible))).powi(2);
                brier_pairs += 1;
            }
            if let Some(selection) = &row.selection {
                gains.push((row.predicted_gain, selection.delta));
            }
        }
        let mut ranked: Vec<_> = self.rows.iter().collect();
        ranked.sort_by(|a, b| {
            b.predicted_gain
                .partial_cmp(&a.predicted_gain)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.proposal_index.cmp(&b.proposal_index))
        });
        let top_k: Vec<_> = ranked[..k]
            .iter()
            .filter_map(|r| r.selection.as_ref().and_then(|s| s.eligible))
            .collect();
        Ok(ProposalOutcomeMetrics {
            rows: self.rows.len(),
            missing_priors: self.rows.iter().filter(|r| r.prior.is_none()).count(),
            missing_labels: self
                .rows
                .iter()
                .filter(|r| r.selection.as_ref().is_none_or(|s| s.eligible.is_none()))
                .count(),
            brier_pairs,
            brier: if brier_pairs == 0 {
                None
            } else {
                Some(squared_error / count(brier_pairs)?)
            },
            gain_pairs: gains.len(),
            spearman: spearman(&gains)?,
            k,
            top_k_observed: top_k.len(),
            precision_at_k: if top_k.len() == k {
                Some(count(top_k.iter().filter(|v| **v).count())? / count(k)?)
            } else {
                None
            },
        })
    }
}
/// Converts bounded counts exactly so metrics never silently round large denominators.
fn count(value: usize) -> Result<f64> {
    Ok(f64::from(
        u32::try_from(value).map_err(|_| Error::InvalidEvidence)?,
    ))
}
/// Assigns average one-based ranks to numeric ties, including signed zero.
fn ranks(values: &[f64]) -> Result<Vec<f64>> {
    let mut ordered: Vec<_> = values.iter().enumerate().collect();
    ordered.sort_by(|a, b| a.1.total_cmp(b.1));
    let mut result = vec![0.0; values.len()];
    let mut start = 0;
    while start < ordered.len() {
        let mut end = start + 1;
        while end < ordered.len()
            && ordered[start].1.partial_cmp(ordered[end].1) == Some(Ordering::Equal)
        {
            end += 1;
        }
        let rank = (count(start)? + count(end)? + 1.0) / 2.0;
        for &(index, _) in &ordered[start..end] {
            result[index] = rank;
        }
        start = end;
    }
    Ok(result)
}
/// Computes Pearson correlation of tied ranks without an invalid constant-vector result.
fn spearman(pairs: &[(f64, f64)]) -> Result<Option<f64>> {
    if pairs.len() < 2 {
        return Ok(None);
    }
    let x = ranks(&pairs.iter().map(|p| p.0).collect::<Vec<_>>())?;
    let y = ranks(&pairs.iter().map(|p| p.1).collect::<Vec<_>>())?;
    let mean = count(pairs.len())?.midpoint(1.0);
    let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
    for (x, y) in x.iter().zip(y) {
        let a = x - mean;
        let b = y - mean;
        xx += a * a;
        yy += b * b;
        xy += a * b;
    }
    if xx <= 0.0 || yy <= 0.0 {
        return Ok(None);
    }
    Ok(Some((xy / (xx.sqrt() * yy.sqrt())).clamp(-1.0, 1.0)))
}
/// Refuses malformed public/deserialized inputs before computing any statistic.
fn validate(dataset: &ProposalOutcomeDataset, k: usize) -> Result<()> {
    if dataset.version != 1
        || !digest(&dataset.fingerprint)
        || k == 0
        || k > dataset.rows.len()
        || dataset
            .model
            .as_ref()
            .is_some_and(|model| !model_valid(model))
        || dataset.generation.as_ref().is_some_and(|a| {
            a.epoch != dataset.epoch
                || a.fingerprint != dataset.fingerprint
                || a.mode != dataset.mode
        })
        || dataset.rows.iter().enumerate().any(|(i, r)| {
            r.proposal_index != i
                || !r.predicted_gain.is_finite()
                || r.prior.is_some_and(|p| !probability(p))
                || r.formatter_run_id.as_ref().is_some_and(|id| !digest(id))
                || r.occurrence.is_some() != r.formatter_run_id.is_some()
                || r.selection.as_ref().is_some_and(|s| {
                    !s.delta.is_finite()
                        || !s.lower.is_finite()
                        || !s.upper.is_finite()
                        || s.lower > s.upper
                })
                || match (&r.selection, r.label) {
                    (Some(s), ProposalLabel::Observed) => s.eligible.is_none(),
                    (Some(s), ProposalLabel::Inconclusive) => s.eligible.is_some(),
                    (None, ProposalLabel::Observed | ProposalLabel::Inconclusive)
                    | (Some(_), _) => true,
                    (None, _) => false,
                }
        })
    {
        return Err(Error::InvalidEvidence);
    }
    count(dataset.rows.len())?;
    Ok(())
}
