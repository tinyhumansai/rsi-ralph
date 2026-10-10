//! Frozen-anchor H scoring, paired cluster bootstrap, and anchor monitoring.
//!
//! Bootstrap draws resample tasks within families and shared seed clusters,
//! preserving paired arms. Intervals are conditional on host-supplied health
//! rates and immutable family references, not calibrated sequential guarantees.

use std::collections::BTreeMap;

use crate::{Budget, EvalDecision, EvalPolicy, EvalRun, Result};

mod types;
pub use types::{
    AnchorLedger, AnchorSignal, FamilyAnchor, ScoreComparison, ScoreEstimate, ScoreHealth,
    ScorePenalties, ScoringPolicy,
};

impl ScoringPolicy {
    /// Freeze reference anchors, penalty coefficients, and bootstrap settings.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidEvaluation`] for duplicate/empty families,
    /// invalid references, nonpositive weights, negative/nonfinite coefficients
    /// or limits, or fewer than 200 bootstrap resamples.
    pub fn new(
        anchors: Vec<FamilyAnchor>,
        penalties: ScorePenalties,
        margin: f64,
        cost_tolerance: f64,
        resamples: u32,
        bootstrap_seed: u64,
    ) -> Result<Self> {
        if anchors.is_empty()
            || resamples < 200
            || [
                penalties.false_claims,
                penalties.tamper,
                penalties.cost,
                margin,
                cost_tolerance,
            ]
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        {
            return invalid();
        }
        let mut families = BTreeMap::new();
        for anchor in anchors {
            if anchor.family.trim().is_empty()
                || !anchor.baseline.is_finite()
                || !(0.0..=1.0).contains(&anchor.baseline)
                || !anchor.reference.is_finite()
                || !(0.0..=1.0).contains(&anchor.reference)
                || anchor.reference <= anchor.baseline
                || !anchor.weight.is_finite()
                || anchor.weight <= 0.0
                || !anchor.regression_limit.is_finite()
                || anchor.regression_limit < 0.0
                || families.insert(anchor.family.clone(), anchor).is_some()
            {
                return invalid();
            }
        }
        Ok(Self {
            anchors: families,
            penalties,
            margin,
            cost_tolerance,
            resamples,
            bootstrap_seed,
        })
    }
}

/// Compute normalized H and a deterministic 95% percentile-bootstrap interval.
///
/// Task checkpoints are scored before this call, for example by
/// [`crate::QualityRubric`]. Family weights apply before fractional IQM trimming.
/// Host health rates and reference anchors remain fixed across resamples.
///
/// # Errors
/// Returns [`crate::Error::InvalidEvaluation`] for invalid/nonfinite telemetry,
/// unknown/missing families, malformed run matrices, cap violations, or numeric
/// overflow in normalization, penalties, or bootstrap estimates.
pub fn score(
    runs: &[EvalRun],
    health: &ScoreHealth,
    budget: &Budget,
    policy: &ScoringPolicy,
) -> Result<ScoreEstimate> {
    validate_health(health)?;
    let evidence = evidence(runs, budget, policy)?;
    let per_family = normalized(&evidence, policy, None)?;
    let value = hive_score(&per_family, health, policy)?;
    let mut rng = Rng(policy.bootstrap_seed);
    let mut values = Vec::new();
    for _ in 0..policy.resamples {
        let draw = draw(&evidence, &mut rng)?;
        values.push(hive_score(
            &normalized(&evidence, policy, Some(&draw))?,
            health,
            policy,
        )?);
    }
    let (lower, upper) = interval(&mut values)?;
    Ok(ScoreEstimate {
        value,
        lower,
        upper,
        per_family,
        health: health.clone(),
    })
}

/// Compare paired normalized H with shared task-and-seed bootstrap draws.
///
/// Acceptance is eligibility only; independent confirmation and human approval
/// remain host responsibilities. Health rates are host telemetry; candidate
/// cost ratio must agree with observed token totals relative to the incumbent.
///
/// # Errors
/// Returns [`crate::Error::InvalidEvaluation`] for invalid evidence or telemetry,
/// mismatched paired keys, inconsistent cost ratios, or numeric overflow.
pub fn compare_h(
    incumbent: &[EvalRun],
    candidate: &[EvalRun],
    incumbent_health: &ScoreHealth,
    candidate_health: &ScoreHealth,
    budget: &Budget,
    policy: &ScoringPolicy,
) -> Result<ScoreComparison> {
    crate::compare(incumbent, candidate, budget, &EvalPolicy::default())?;
    validate_health(incumbent_health)?;
    validate_health(candidate_health)?;
    validate_cost(incumbent, candidate, incumbent_health, candidate_health)?;
    let inc = evidence(incumbent, budget, policy)?;
    let cand = evidence(candidate, budget, policy)?;
    let inc_families = normalized(&inc, policy, None)?;
    let cand_families = normalized(&cand, policy, None)?;
    let inc_value = hive_score(&inc_families, incumbent_health, policy)?;
    let cand_value = hive_score(&cand_families, candidate_health, policy)?;
    let (mut inc_values, mut cand_values, mut deltas) =
        bootstrap_pair(&inc, &cand, incumbent_health, candidate_health, policy)?;
    let (inc_lower, inc_upper) = interval(&mut inc_values)?;
    let (cand_lower, cand_upper) = interval(&mut cand_values)?;
    let (lower, upper) = interval(&mut deltas)?;
    let mut decision = decision(
        &inc_families,
        &cand_families,
        incumbent_health,
        candidate_health,
        lower,
        upper,
        policy,
    )?;
    if policy.cost_tolerance == 0.0 && exact_tokens(candidate) > exact_tokens(incumbent) {
        decision = EvalDecision::Reject {
            reason: "token cost exceeded zero tolerance".into(),
        };
    }
    let delta = cand_value - inc_value;
    if !delta.is_finite() {
        return invalid();
    }
    Ok(ScoreComparison {
        incumbent: ScoreEstimate {
            value: inc_value,
            lower: inc_lower,
            upper: inc_upper,
            per_family: inc_families,
            health: incumbent_health.clone(),
        },
        candidate: ScoreEstimate {
            value: cand_value,
            lower: cand_lower,
            upper: cand_upper,
            per_family: cand_families,
            health: candidate_health.clone(),
        },
        delta,
        lower,
        upper,
        decision,
    })
}

type Evidence = BTreeMap<String, Vec<Vec<f64>>>;

fn evidence(runs: &[EvalRun], budget: &Budget, policy: &ScoringPolicy) -> Result<Evidence> {
    crate::compare(runs, runs, budget, &EvalPolicy::default())?;
    let mut tasks: BTreeMap<(&str, &str), BTreeMap<u64, f64>> = BTreeMap::new();
    for run in runs {
        tasks
            .entry((&run.family, &run.task))
            .or_default()
            .insert(run.seed, run.quality);
    }
    let mut families: Evidence = BTreeMap::new();
    for ((family, _), seeds) in tasks {
        families
            .entry(family.into())
            .or_default()
            .push(seeds.into_values().collect());
    }
    if families.keys().ne(policy.anchors.keys()) {
        return invalid();
    }
    Ok(families)
}

#[derive(Debug)]
struct Draw {
    seeds: Vec<usize>,
    tasks: BTreeMap<String, Vec<usize>>,
}

fn draw(evidence: &Evidence, rng: &mut Rng) -> Result<Draw> {
    let seed_count = evidence
        .values()
        .next()
        .and_then(|tasks| tasks.first())
        .ok_or(crate::Error::InvalidEvaluation)?
        .len();
    let seeds = (0..seed_count)
        .map(|_| rng.index(seed_count))
        .collect::<Result<Vec<_>>>()?;
    let tasks = evidence
        .iter()
        .map(|(family, tasks)| {
            let indices = (0..tasks.len())
                .map(|_| rng.index(tasks.len()))
                .collect::<Result<Vec<_>>>()?;
            Ok((family.clone(), indices))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    Ok(Draw { seeds, tasks })
}

fn normalized(
    evidence: &Evidence,
    policy: &ScoringPolicy,
    draw: Option<&Draw>,
) -> Result<BTreeMap<String, f64>> {
    evidence
        .iter()
        .map(|(family, tasks)| {
            let mut total = 0.0;
            let mut count = 0.0;
            if let Some(draw) = draw {
                for index in draw
                    .tasks
                    .get(family)
                    .ok_or(crate::Error::InvalidEvaluation)?
                {
                    let seeds = tasks.get(*index).ok_or(crate::Error::InvalidEvaluation)?;
                    for seed in &draw.seeds {
                        total += seeds.get(*seed).ok_or(crate::Error::InvalidEvaluation)?;
                        count += 1.0;
                    }
                }
            } else {
                for seeds in tasks {
                    for quality in seeds {
                        total += quality;
                        count += 1.0;
                    }
                }
            }
            let anchor = policy
                .anchors
                .get(family)
                .ok_or(crate::Error::InvalidEvaluation)?;
            let normalized =
                (total / count - anchor.baseline) / (anchor.reference - anchor.baseline);
            if !normalized.is_finite() {
                return invalid();
            }
            Ok((family.clone(), normalized))
        })
        .collect()
}

fn hive_score(
    families: &BTreeMap<String, f64>,
    health: &ScoreHealth,
    policy: &ScoringPolicy,
) -> Result<f64> {
    let weighted = families
        .iter()
        .map(|(family, quality)| {
            let weight = policy
                .anchors
                .get(family)
                .ok_or(crate::Error::InvalidEvaluation)?
                .weight;
            let weighted = quality * weight;
            if !weighted.is_finite() {
                return invalid();
            }
            Ok(weighted)
        })
        .collect::<Result<Vec<_>>>()?;
    let score = iqm(&weighted)
        - policy.penalties.false_claims * health.false_claim_rate
        - policy.penalties.tamper * health.tamper_rate
        - policy.penalties.cost * (health.cost_ratio - 1.0).max(0.0);
    if !score.is_finite() {
        return invalid();
    }
    Ok(score)
}

fn iqm(values: &[f64]) -> f64 {
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    let count = values.iter().fold(0.0, |count, _| count + 1.0);
    let lower = count * 0.25;
    let upper = count * 0.75;
    let mut position: f64 = 0.0;
    let mut total = 0.0;
    for value in values {
        let overlap = ((position + 1.0).min(upper) - position.max(lower)).max(0.0);
        total += value * overlap / (upper - lower);
        position += 1.0;
    }
    total
}

fn bootstrap_pair(
    inc: &Evidence,
    cand: &Evidence,
    inc_health: &ScoreHealth,
    cand_health: &ScoreHealth,
    policy: &ScoringPolicy,
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let mut rng = Rng(policy.bootstrap_seed);
    let (mut inc_values, mut cand_values, mut deltas) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..policy.resamples {
        let draw = draw(inc, &mut rng)?;
        let i = hive_score(&normalized(inc, policy, Some(&draw))?, inc_health, policy)?;
        let c = hive_score(&normalized(cand, policy, Some(&draw))?, cand_health, policy)?;
        if !(c - i).is_finite() {
            return invalid();
        }
        inc_values.push(i);
        cand_values.push(c);
        deltas.push(c - i);
    }
    Ok((inc_values, cand_values, deltas))
}

fn interval(values: &mut [f64]) -> Result<(f64, f64)> {
    values.sort_by(f64::total_cmp);
    Ok((quantile(values, 25)?, quantile(values, 975)?))
}

fn quantile(values: &[f64], numerator: u64) -> Result<f64> {
    let length = u64::try_from(values.len()).map_err(|_| crate::Error::InvalidEvaluation)?;
    let position = length
        .checked_sub(1)
        .and_then(|length| length.checked_mul(numerator))
        .ok_or(crate::Error::InvalidEvaluation)?;
    let index = usize::try_from(position / 1000).map_err(|_| crate::Error::InvalidEvaluation)?;
    let fraction =
        f64::from(u32::try_from(position % 1000).map_err(|_| crate::Error::InvalidEvaluation)?)
            / 1000.0;
    let lower = *values.get(index).ok_or(crate::Error::InvalidEvaluation)?;
    let upper = values.get(index + 1).copied().unwrap_or(lower);
    Ok(if lower.total_cmp(&upper).is_eq() {
        lower
    } else {
        lower * (1.0 - fraction) + upper * fraction
    })
}

#[derive(Debug)]
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
    fn index(&mut self, bound: usize) -> Result<usize> {
        let bound = u64::try_from(bound).map_err(|_| crate::Error::InvalidEvaluation)?;
        if bound == 0 {
            return invalid();
        }
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next();
            if value >= threshold {
                return usize::try_from(value % bound).map_err(|_| crate::Error::InvalidEvaluation);
            }
        }
    }
}

fn validate_health(health: &ScoreHealth) -> Result<()> {
    if [health.false_claim_rate, health.tamper_rate]
        .iter()
        .any(|rate| !rate.is_finite() || !(0.0..=1.0).contains(rate))
        || !health.cost_ratio.is_finite()
        || health.cost_ratio < 0.0
    {
        return invalid();
    }
    Ok(())
}

fn exact_tokens(runs: &[EvalRun]) -> u128 {
    runs.iter().map(|run| u128::from(run.tokens)).sum()
}

fn token_total(runs: &[EvalRun]) -> f64 {
    runs.iter()
        .map(|run| {
            run.tokens
                .to_be_bytes()
                .iter()
                .fold(0.0, |value, byte| value * 256.0 + f64::from(*byte))
        })
        .sum()
}

fn validate_cost(
    inc: &[EvalRun],
    cand: &[EvalRun],
    inc_health: &ScoreHealth,
    cand_health: &ScoreHealth,
) -> Result<()> {
    let inc_tokens = token_total(inc);
    let cand_tokens = token_total(cand);
    let expected = if inc_tokens == 0.0 {
        if cand_tokens != 0.0 {
            return invalid();
        }
        1.0
    } else {
        cand_tokens / inc_tokens
    };
    if (inc_health.cost_ratio - 1.0).abs() > 1e-12
        || (cand_health.cost_ratio - expected).abs() > 1e-12 * expected.max(1.0)
    {
        return invalid();
    }
    Ok(())
}

fn decision(
    inc: &BTreeMap<String, f64>,
    cand: &BTreeMap<String, f64>,
    inc_health: &ScoreHealth,
    cand_health: &ScoreHealth,
    lower: f64,
    upper: f64,
    policy: &ScoringPolicy,
) -> Result<EvalDecision> {
    let mut family_regression = false;
    for (family, anchor) in &policy.anchors {
        let candidate = cand.get(family).ok_or(crate::Error::InvalidEvaluation)?;
        let incumbent = inc.get(family).ok_or(crate::Error::InvalidEvaluation)?;
        family_regression |= candidate < &(incumbent - anchor.regression_limit);
    }
    let reason = if family_regression {
        Some("normalized family regressed")
    } else if cand_health.false_claim_rate > inc_health.false_claim_rate
        || cand_health.tamper_rate > inc_health.tamper_rate
    {
        Some("health rate regressed")
    } else if cand_health.cost_ratio > 1.0 + policy.cost_tolerance {
        Some("cost ratio exceeded tolerance")
    } else if upper < 0.0 {
        Some("paired H interval rules out improvement")
    } else {
        None
    };
    Ok(if let Some(reason) = reason {
        EvalDecision::Reject {
            reason: reason.into(),
        }
    } else if lower > policy.margin {
        EvalDecision::Accept {
            reason: "paired H lower bound exceeds margin".into(),
        }
    } else {
        EvalDecision::Inconclusive {
            reason: "paired H interval does not establish practical improvement".into(),
        }
    })
}

impl AnchorLedger {
    /// Monitor consecutive epochs without changing any active version.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidEvaluation`] for fewer than two required
    /// epochs or negative/nonfinite monitoring margins.
    pub fn new(required_epochs: usize, regression_margin: f64, gap_margin: f64) -> Result<Self> {
        if required_epochs < 2
            || [regression_margin, gap_margin]
                .iter()
                .any(|margin| !margin.is_finite() || *margin < 0.0)
        {
            return invalid();
        }
        Ok(Self {
            required_epochs,
            regression_margin,
            gap_margin,
            best: None,
            initial_gap: None,
            last_epoch: None,
            regression_streak: 0,
            gap_streak: 0,
        })
    }

    /// Record a selection and frozen-anchor estimate from one epoch.
    ///
    /// Regression requires anchor upper bounds below the best previous anchor
    /// lower bound minus margin. Gap widening uses selection lower minus anchor
    /// upper, relative to the first epoch's point-estimate gap. Skipped epochs
    /// reset streaks. Scores must use the same immutable references and suite.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidEvaluation`] for nonincreasing epochs,
    /// malformed/nonfinite estimates, or inconsistent family identities.
    pub fn record(
        &mut self,
        epoch: u64,
        selection: &ScoreEstimate,
        anchor: &ScoreEstimate,
    ) -> Result<AnchorSignal> {
        validate_estimate(selection)?;
        validate_estimate(anchor)?;
        if selection.per_family.keys().ne(anchor.per_family.keys())
            || self.last_epoch.is_some_and(|last| epoch <= last)
            || self
                .best
                .as_ref()
                .is_some_and(|(_, best)| best.per_family.keys().ne(anchor.per_family.keys()))
        {
            return invalid();
        }
        let gap = selection.value - anchor.value;
        if !gap.is_finite() {
            return invalid();
        }
        if self
            .last_epoch
            .is_some_and(|last| last.checked_add(1) != Some(epoch))
        {
            self.regression_streak = 0;
            self.gap_streak = 0;
        }
        let initial_gap = *self.initial_gap.get_or_insert(gap);
        let regression = self
            .best
            .as_ref()
            .is_some_and(|(_, best)| anchor.upper < best.lower - self.regression_margin);
        let widening_gap = selection.lower - anchor.upper > initial_gap + self.gap_margin;
        self.regression_streak = if regression {
            self.regression_streak.saturating_add(1)
        } else {
            0
        };
        self.gap_streak = if widening_gap {
            self.gap_streak.saturating_add(1)
        } else {
            0
        };
        if self
            .best
            .as_ref()
            .is_none_or(|(_, best)| anchor.value > best.value)
        {
            self.best = Some((epoch, anchor.clone()));
        }
        self.last_epoch = Some(epoch);
        let best_epoch = self.best.as_ref().ok_or(crate::Error::InvalidEvaluation)?.0;
        Ok(AnchorSignal {
            regression: self.regression_streak >= self.required_epochs,
            widening_gap: self.gap_streak >= self.required_epochs,
            best_epoch,
        })
    }
}

fn validate_estimate(estimate: &ScoreEstimate) -> Result<()> {
    validate_health(&estimate.health)?;
    if [estimate.value, estimate.lower, estimate.upper]
        .iter()
        .any(|value| !value.is_finite())
        || estimate.lower > estimate.upper
        || estimate.per_family.is_empty()
        || estimate
            .per_family
            .iter()
            .any(|(family, value)| family.trim().is_empty() || !value.is_finite())
    {
        return invalid();
    }
    Ok(())
}

fn invalid<T>() -> Result<T> {
    Err(crate::Error::InvalidEvaluation)
}

#[cfg(test)]
mod test;
