//! Pure paired evaluation; the host owns execution and confirmation orchestration.
//!
//! Families receive equal weight, tasks receive equal weight within each family,
//! and uncertainty is estimated across seed-level means rather than treating
//! correlated task outcomes for the same seed as independent observations.

use std::collections::{BTreeMap, BTreeSet};

mod types;
pub use types::{
    Budget, Comparison, EvalDecision, EvalPolicy, EvalRun, JudgeCalibration, QualityRubric,
    VerifierTier,
};

/// Compare complete, paired outcomes under the same hard execution cap.
///
/// A clear loser can be rejected on a small held-out screening matrix. Acceptance
/// on this matrix alone does not authorize promotion: the host must run a fresh
/// independent confirmation matrix and obtain human approval.
///
/// # Errors
/// Returns [`crate::Error::InvalidEvaluation`] for invalid policy, nonfinite or
/// out-of-range qualities, overruns, duplicate/mismatched keys, inconsistent
/// families, or an incomplete task-by-seed matrix with fewer than two seeds.
pub fn compare(
    incumbent: &[EvalRun],
    candidate: &[EvalRun],
    budget: &Budget,
    policy: &EvalPolicy,
) -> crate::Result<Comparison> {
    validate_policy(budget, policy)?;
    let incumbent = matrix(incumbent, budget)?;
    let candidate = matrix(candidate, budget)?;
    if incumbent.keys().ne(candidate.keys()) {
        return Err(crate::Error::InvalidEvaluation);
    }
    let mut groups: BTreeMap<(&str, u64), Vec<f64>> = BTreeMap::new();
    let mut health_regression = false;
    let mut incumbent_tokens = 0_u128;
    let mut candidate_tokens = 0_u128;
    for (key, baseline) in &incumbent {
        let proposed = candidate.get(key).ok_or(crate::Error::InvalidEvaluation)?;
        groups
            .entry((key.1, key.2))
            .or_default()
            .push(proposed.quality - baseline.quality);
        health_regression |=
            proposed.false_claims > baseline.false_claims || proposed.tamper > baseline.tamper;
        incumbent_tokens += u128::from(baseline.tokens);
        candidate_tokens += u128::from(proposed.tokens);
    }
    let mut seeds: BTreeMap<u64, Vec<f64>> = BTreeMap::new();
    let mut families: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for ((family, seed), gains) in groups {
        let gain = mean(&gains);
        seeds.entry(seed).or_default().push(gain);
        families.entry(family).or_default().push(gain);
    }
    let paired: Vec<f64> = seeds.values().map(|values| mean(values)).collect();
    let gain = mean(&paired);
    let count = paired.iter().fold(0.0, |count, _| count + 1.0);
    let variance = paired
        .iter()
        .map(|value| (value - gain).powi(2))
        .sum::<f64>()
        / (count - 1.0);
    let standard_error = (variance / count).sqrt();
    let lower = gain - policy.standard_error_multiplier * standard_error;
    let upper = gain + policy.standard_error_multiplier * standard_error;
    if !lower.is_finite() || !upper.is_finite() {
        return Err(crate::Error::InvalidEvaluation);
    }
    let family_regression = families
        .values()
        .any(|gains| mean(gains) < -policy.family_regression_limit);
    let cost_regression = if policy.cost_tolerance == 0.0 {
        candidate_tokens > incumbent_tokens
    } else {
        integer_float(candidate_tokens)
            > integer_float(incumbent_tokens) * (1.0 + policy.cost_tolerance)
    };
    let decision = if health_regression {
        reject("false claims or tampering increased")
    } else if family_regression {
        reject("task-family quality regressed")
    } else if cost_regression {
        reject("token cost exceeded tolerance")
    } else if lower > policy.margin {
        EvalDecision::Accept {
            reason: "paired lower bound exceeds practical margin".into(),
        }
    } else if upper <= policy.margin {
        reject("paired upper bound does not exceed practical margin")
    } else {
        EvalDecision::Inconclusive {
            reason: "paired interval overlaps practical margin".into(),
        }
    };
    Ok(Comparison {
        gain,
        standard_error,
        lower,
        upper,
        decision,
    })
}

/// Verify that selection and confirmation reuse neither tasks nor random seeds.
///
/// This checks identities only; the host must still compare the incumbent and
/// candidate independently within each phase and keep private tasks private.
///
/// # Errors
/// Returns [`crate::Error::InvalidEvaluation`] for empty evidence, empty task
/// identifiers, overlapping task identities, or overlapping random seeds.
pub fn ensure_independent(selection: &[EvalRun], confirmation: &[EvalRun]) -> crate::Result<()> {
    if selection.is_empty()
        || confirmation.is_empty()
        || selection
            .iter()
            .chain(confirmation)
            .any(|run| run.task.trim().is_empty())
    {
        return Err(crate::Error::InvalidEvaluation);
    }
    let tasks: BTreeSet<&str> = selection.iter().map(|run| run.task.as_str()).collect();
    let seeds: BTreeSet<u64> = selection.iter().map(|run| run.seed).collect();
    if confirmation
        .iter()
        .any(|run| tasks.contains(run.task.as_str()) || seeds.contains(&run.seed))
    {
        return Err(crate::Error::InvalidEvaluation);
    }
    Ok(())
}

fn validate_policy(budget: &Budget, policy: &EvalPolicy) -> crate::Result<()> {
    if budget.max_tokens == 0
        || [
            policy.margin,
            policy.standard_error_multiplier,
            policy.family_regression_limit,
            policy.cost_tolerance,
        ]
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
        || policy.standard_error_multiplier < 2.0
        || policy.margin > 1.0
        || policy.family_regression_limit > 1.0
    {
        return Err(crate::Error::InvalidEvaluation);
    }
    Ok(())
}

type Matrix<'a> = BTreeMap<(&'a str, &'a str, u64), &'a EvalRun>;

fn matrix<'a>(runs: &'a [EvalRun], budget: &Budget) -> crate::Result<Matrix<'a>> {
    if runs.is_empty() {
        return Err(crate::Error::InvalidEvaluation);
    }
    let mut matrix = BTreeMap::new();
    let mut tasks: BTreeMap<&str, (&str, BTreeSet<u64>)> = BTreeMap::new();
    let all_seeds: BTreeSet<u64> = runs.iter().map(|run| run.seed).collect();
    if all_seeds.len() < 2 {
        return Err(crate::Error::InvalidEvaluation);
    }
    for run in runs {
        if run.task.trim().is_empty()
            || run.family.trim().is_empty()
            || !run.quality.is_finite()
            || !(0.0..=1.0).contains(&run.quality)
            || run.tokens > budget.max_tokens
            || matrix
                .insert((run.task.as_str(), run.family.as_str(), run.seed), run)
                .is_some()
        {
            return Err(crate::Error::InvalidEvaluation);
        }
        let (family, seeds) = tasks
            .entry(run.task.as_str())
            .or_insert_with(|| (run.family.as_str(), BTreeSet::new()));
        if *family != run.family {
            return Err(crate::Error::InvalidEvaluation);
        }
        seeds.insert(run.seed);
    }
    if tasks.values().any(|(_, seeds)| *seeds != all_seeds) {
        return Err(crate::Error::InvalidEvaluation);
    }
    Ok(matrix)
}

fn mean(values: &[f64]) -> f64 {
    let count = values.iter().fold(0.0, |count, _| count + 1.0);
    values.iter().sum::<f64>() / count
}

fn reject(reason: &str) -> EvalDecision {
    EvalDecision::Reject {
        reason: reason.into(),
    }
}

// Convert exact integer totals without lossy casts; zero-tolerance cost checks
// use integers directly so a single extra token is never hidden by rounding.
fn integer_float(value: u128) -> f64 {
    value
        .to_be_bytes()
        .iter()
        .fold(0.0, |total, byte| total * 256.0 + f64::from(*byte))
}

impl JudgeCalibration {
    /// Calibrate binary judge labels against matching human gold labels.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidEvaluation`] for unequal or fewer than two
    /// labels, a threshold outside `[0,1]`, or a degenerate constant-label panel
    /// for which Cohen's kappa is undefined.
    pub fn from_labels(judge: &[bool], human: &[bool], threshold: f64) -> crate::Result<Self> {
        if judge.len() != human.len()
            || judge.len() < 2
            || !threshold.is_finite()
            || !(0.0..=1.0).contains(&threshold)
        {
            return Err(crate::Error::InvalidEvaluation);
        }
        let count = judge.iter().fold(0.0, |n, _| n + 1.0);
        let judge_positive = judge
            .iter()
            .filter(|label| **label)
            .fold(0.0, |n, _| n + 1.0)
            / count;
        let human_positive = human
            .iter()
            .filter(|label| **label)
            .fold(0.0, |n, _| n + 1.0)
            / count;
        let observed = judge
            .iter()
            .zip(human)
            .filter(|(j, h)| j == h)
            .fold(0.0, |n, _| n + 1.0)
            / count;
        let expected =
            judge_positive * human_positive + (1.0 - judge_positive) * (1.0 - human_positive);
        if expected >= 1.0 {
            return Err(crate::Error::InvalidEvaluation);
        }
        Ok(Self {
            agreement: (observed - expected) / (1.0 - expected),
            threshold,
        })
    }

    /// Measured Cohen's kappa; negative values indicate worse than chance.
    #[must_use]
    pub fn agreement(&self) -> f64 {
        self.agreement
    }

    fn weight(&self) -> crate::Result<f64> {
        if !self.agreement.is_finite()
            || !(-1.0..=1.0).contains(&self.agreement)
            || !self.threshold.is_finite()
            || !(0.0..=1.0).contains(&self.threshold)
        {
            return Err(crate::Error::InvalidEvaluation);
        }
        Ok(if self.agreement >= self.threshold {
            self.agreement.max(0.0)
        } else {
            0.0
        })
    }
}

impl VerifierTier {
    fn weight(&self) -> crate::Result<f64> {
        match self {
            Self::Proof => Ok(1.0),
            Self::Execution => Ok(0.9),
            Self::Schema | Self::HeldOutReference => Ok(0.7),
            Self::RubricJudge(calibration) => calibration.weight(),
            Self::Preference(calibration) => Ok(0.5 * calibration.weight()?),
        }
    }
}

impl QualityRubric {
    /// Freeze the goal weight and eval-owned checkpoint verifier tiers.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidEvaluation`] for alpha outside `[0,1]`,
    /// invalid judge calibration, or no checkpoint with positive weight.
    pub fn new(alpha: f64, checkpoints: Vec<VerifierTier>) -> crate::Result<Self> {
        let rubric = Self { alpha, checkpoints };
        rubric.weights()?;
        Ok(rubric)
    }

    /// Compute goal plus weighted checkpoint quality in `[0,1]`.
    ///
    /// Exceeding the budget removes terminal-goal credit. Checkpoint passes
    /// must describe verifications completed before the cap was exhausted.
    /// Overrun evidence still cannot enter [`compare`].
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidEvaluation`] for a zero budget, an invalid
    /// deserialized rubric, or a checkpoint count different from the frozen one.
    pub fn score(
        &self,
        goal_verified: bool,
        tokens: u64,
        budget: &Budget,
        passes: &[bool],
    ) -> crate::Result<f64> {
        if budget.max_tokens == 0 || passes.len() != self.checkpoints.len() {
            return Err(crate::Error::InvalidEvaluation);
        }
        let weights = self.weights()?;
        let total = weights.iter().sum::<f64>();
        let partial = weights
            .iter()
            .zip(passes)
            .filter(|(_, passed)| **passed)
            .map(|(weight, _)| *weight)
            .sum::<f64>()
            / total;
        let goal = f64::from(u8::from(goal_verified && tokens <= budget.max_tokens));
        Ok(self.alpha * goal + (1.0 - self.alpha) * partial)
    }

    fn weights(&self) -> crate::Result<Vec<f64>> {
        if !self.alpha.is_finite() || !(0.0..=1.0).contains(&self.alpha) {
            return Err(crate::Error::InvalidEvaluation);
        }
        let weights = self
            .checkpoints
            .iter()
            .map(VerifierTier::weight)
            .collect::<crate::Result<Vec<_>>>()?;
        let total = weights.iter().sum::<f64>();
        if !total.is_finite() || total <= 0.0 {
            return Err(crate::Error::InvalidEvaluation);
        }
        Ok(weights)
    }
}

#[cfg(test)]
mod test;
