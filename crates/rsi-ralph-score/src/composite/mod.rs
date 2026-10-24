//! Combining scorers, and scoring a part of a subject.
//!
//! [`Composite`] takes a weighted mean of its members' per-seed qualities. Its
//! eligible qualities are the weighted mean of the *eligible members only*,
//! with their weights renormalized, so a heavily weighted uncalibrated judge
//! can move the combined score but the eligible evidence must still show an
//! improvement by itself (see [`crate::decide`]). A blend reports its interval
//! in quality units; only a single-member composite keeps a member's metric.
//!
//! [`Projected`] scores one view of a richer subject, for example the metric
//! values of a candidate that also has source text, so members of one
//! composite can each read the part they understand.
use crate::{Assessment, Error, Result, Scorer};
use rsi_ralph_types::Goal;

/// Tolerance on the sum of weights.
const WEIGHT_EPSILON: f64 = 1e-9;

type Member<'a, S> = (f64, Box<dyn Scorer<S> + Send + Sync + 'a>);

/// A weighted combination of scorers over the same subject.
pub struct Composite<'a, S: ?Sized> {
    members: Vec<Member<'a, S>>,
}

impl<'a, S: ?Sized> Composite<'a, S> {
    /// An empty composite; add members with [`Composite::with`].
    #[must_use]
    pub fn new() -> Self {
        Self { members: vec![] }
    }

    /// Adds `scorer` with `weight`. Weights must be positive and sum to one
    /// by the time the composite assesses.
    #[must_use]
    pub fn with(mut self, weight: f64, scorer: impl Scorer<S> + Send + Sync + 'a) -> Self {
        self.members.push((weight, Box::new(scorer)));
        self
    }
}

impl<S: ?Sized> Default for Composite<'_, S> {
    fn default() -> Self {
        Self::new()
    }
}

fn weighted(parts: &[(f64, &Vec<f64>)], seeds: usize) -> Vec<f64> {
    let total: f64 = parts.iter().map(|(w, _)| w).sum();
    (0..seeds)
        .map(|i| parts.iter().map(|(w, q)| w * q[i]).sum::<f64>() / total)
        .collect()
}

impl<S: ?Sized> Scorer<S> for Composite<'_, S> {
    /// # Errors
    ///
    /// Returns [`Error::InvalidEvaluation`] for no members, a non-positive or
    /// non-finite weight, weights that do not sum to one, or a member whose
    /// assessment does not have one quality (and eligible quality) per seed
    /// in `[0, 1]`; and any member's error. Token totals saturate.
    fn assess(&self, subject: &S, goal: &Goal, seeds: &[u64]) -> Result<Assessment> {
        let sum: f64 = self.members.iter().map(|(w, _)| w).sum();
        if self.members.is_empty()
            || self
                .members
                .iter()
                .any(|(w, _)| !w.is_finite() || *w <= 0.0)
            || (sum - 1.0).abs() > WEIGHT_EPSILON
        {
            return Err(Error::InvalidEvaluation);
        }
        let mut assessments = Vec::with_capacity(self.members.len());
        for (weight, scorer) in &self.members {
            let assessment = scorer.assess(subject, goal, seeds)?;
            // Checked before blending: a weighted mean can hide an
            // out-of-range member inside [0, 1].
            let valid =
                |q: &Vec<f64>| q.len() == seeds.len() && q.iter().all(|v| (0.0..=1.0).contains(v));
            if !valid(&assessment.qualities)
                || assessment.eligible.as_ref().is_some_and(|e| !valid(e))
            {
                return Err(Error::InvalidEvaluation);
            }
            assessments.push((*weight, assessment));
        }
        let all: Vec<(f64, &Vec<f64>)> = assessments
            .iter()
            .map(|(w, a)| (*w, &a.qualities))
            .collect();
        let eligible: Vec<(f64, &Vec<f64>)> = assessments
            .iter()
            .filter_map(|(w, a)| a.eligible.as_ref().map(|e| (*w, e)))
            .collect();
        let rationale: Vec<&str> = assessments
            .iter()
            .filter_map(|(_, a)| a.rationale.as_deref())
            .collect();
        Ok(Assessment {
            qualities: weighted(&all, seeds.len()),
            eligible: (!eligible.is_empty()).then(|| weighted(&eligible, seeds.len())),
            // A blend decides on blended qualities; a metric interval beside
            // that decision could contradict it, so only a lone member's
            // metric survives.
            metric: match &assessments[..] {
                [(_, only)] => only.metric.clone(),
                _ => None,
            },
            rationale: (!rationale.is_empty()).then(|| rationale.join("\n\n")),
            tokens: assessments
                .iter()
                .fold(0, |sum: u64, (_, a)| sum.saturating_add(a.tokens)),
        })
    }
}

impl<S: ?Sized> std::fmt::Debug for Composite<'_, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let weights: Vec<f64> = self.members.iter().map(|(w, _)| *w).collect();
        f.debug_struct("Composite")
            .field("weights", &weights)
            .finish()
    }
}

/// Scores the part of `S` that `project` selects, with a scorer of that part.
pub struct Projected<F, Sc> {
    project: F,
    scorer: Sc,
}

impl<F, Sc> Projected<F, Sc> {
    /// Scores `project(subject)` with `scorer`.
    ///
    /// The bound sits here, not only on the [`Scorer`] impl, so a closure
    /// passed straight in gets the borrowed-in, borrowed-out signature it
    /// needs.
    #[must_use]
    pub fn new<S: ?Sized, T: ?Sized>(project: F, scorer: Sc) -> Self
    where
        F: Fn(&S) -> &T,
        Sc: Scorer<T>,
    {
        Self { project, scorer }
    }
}

impl<S: ?Sized, T: ?Sized, F, Sc> Scorer<S> for Projected<F, Sc>
where
    F: Fn(&S) -> &T,
    Sc: Scorer<T>,
{
    fn assess(&self, subject: &S, goal: &Goal, seeds: &[u64]) -> Result<Assessment> {
        self.scorer.assess((self.project)(subject), goal, seeds)
    }
}

impl<F, Sc: std::fmt::Debug> std::fmt::Debug for Projected<F, Sc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Projected")
            .field("scorer", &self.scorer)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test;
