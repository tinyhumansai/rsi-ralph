//! Exact assignment validation, failure reservations, and private anchor evidence.

use super::{AttemptStatus, TrialEvidence, TrialFailure, TrialRequest, TrialSummary, ValidTrial};
use crate::{Error, EvalDecision, Result, ScoringPolicy, compare_h};
use std::collections::BTreeSet;

type TaskShape = BTreeSet<(String, String)>;

pub(super) fn validate_trial(
    evidence: TrialEvidence,
    request: &TrialRequest,
    policy: &ScoringPolicy,
    shape: &mut Option<TaskShape>,
) -> Result<ValidTrial> {
    if evidence.request != *request
        || evidence.attempts.len()
            != usize::try_from(request.attempts_per_trial).map_err(|_| Error::InvalidEvaluation)?
    {
        return Err(Error::InvalidEvaluation);
    }
    let expected: BTreeSet<_> = request.seeds.evaluation_seeds.iter().copied().collect();
    let actual: BTreeSet<_> = evidence.initial_anchor.iter().map(|r| r.seed).collect();
    if expected != actual {
        return Err(Error::InvalidEvaluation);
    }
    let tasks: TaskShape = evidence
        .initial_anchor
        .iter()
        .map(|r| (r.task.clone(), r.family.clone()))
        .collect();
    if shape.as_ref().is_some_and(|s| s != &tasks) {
        return Err(Error::InvalidEvaluation);
    }
    let comparison = compare_h(
        &evidence.initial_anchor,
        &evidence.final_anchor,
        &evidence.initial_health,
        &evidence.final_health,
        &request.per_run_budget,
        policy,
    )?;
    let generation_tokens = sum(evidence.attempts.iter().map(|a| a.generation_tokens))?;
    let evaluation_tokens = sum(evidence
        .attempts
        .iter()
        .map(|a| a.evaluation_tokens)
        .chain(evidence.initial_anchor.iter().map(|r| r.tokens))
        .chain(evidence.final_anchor.iter().map(|r| r.tokens)))?;
    if generation_tokens > request.generation_cap
        || evaluation_tokens > request.evaluation_cap
        || generation_tokens
            .checked_add(evaluation_tokens)
            .ok_or(Error::InvalidEvaluation)?
            == 0
    {
        return Err(Error::InvalidEvaluation);
    }
    let count = |status| {
        u32::try_from(
            evidence
                .attempts
                .iter()
                .filter(|a| a.status == status)
                .count(),
        )
        .map_err(|_| Error::InvalidEvaluation)
    };
    let summary = TrialSummary {
        arm: request.arm,
        ablation: request.ablation,
        proposal_seed: request.seeds.proposal_seed,
        attempts: request.attempts_per_trial,
        accepted: count(AttemptStatus::Accepted)?,
        rejected: count(AttemptStatus::Rejected)?,
        inconclusive: count(AttemptStatus::Inconclusive)?,
        failed: count(AttemptStatus::Failed)?,
        generation_tokens,
        evaluation_tokens,
        verified: true,
        failure: None,
        anchor_gain: Some(comparison.delta),
        anchor_lower: Some(comparison.lower),
        anchor_upper: Some(comparison.upper),
        anchor_accepted: matches!(comparison.decision, EvalDecision::Accept { .. }),
    };
    if shape.is_none() {
        *shape = Some(tasks);
    }
    Ok(ValidTrial {
        evidence,
        summary,
        comparison,
    })
}

pub(super) fn failed(request: &TrialRequest, failure: TrialFailure) -> TrialSummary {
    TrialSummary {
        arm: request.arm,
        ablation: request.ablation,
        proposal_seed: request.seeds.proposal_seed,
        attempts: request.attempts_per_trial,
        accepted: 0,
        rejected: 0,
        inconclusive: 0,
        failed: request.attempts_per_trial,
        generation_tokens: request.generation_cap,
        evaluation_tokens: request.evaluation_cap,
        verified: false,
        failure: Some(failure),
        anchor_gain: None,
        anchor_lower: None,
        anchor_upper: None,
        anchor_accepted: false,
    }
}

pub(super) fn sum(mut values: impl Iterator<Item = u64>) -> Result<u64> {
    values.try_fold(0_u64, |total, value| {
        total.checked_add(value).ok_or(Error::InvalidEvaluation)
    })
}

pub(super) fn compare_arms(
    original: &ValidTrial,
    evolved: &ValidTrial,
    policy: &ScoringPolicy,
) -> Result<crate::ScoreComparison> {
    let mut original_initial = original.evidence.initial_anchor.clone();
    let mut evolved_initial = evolved.evidence.initial_anchor.clone();
    let sort = |runs: &mut Vec<crate::EvalRun>| {
        runs.sort_by(|a, b| (&a.task, &a.family, a.seed).cmp(&(&b.task, &b.family, b.seed)));
    };
    sort(&mut original_initial);
    sort(&mut evolved_initial);
    if original_initial != evolved_initial
        || original.evidence.initial_health != evolved.evidence.initial_health
    {
        return Err(Error::InvalidEvaluation);
    }
    let original_total = sum(original.evidence.final_anchor.iter().map(|r| r.tokens))?;
    let evolved_total = sum(evolved.evidence.final_anchor.iter().map(|r| r.tokens))?;
    let mut inc_health = original.evidence.final_health.clone();
    let mut cand_health = evolved.evidence.final_health.clone();
    inc_health.cost_ratio = 1.;
    cand_health.cost_ratio = if original_total == 0 {
        if evolved_total != 0 {
            return Err(Error::InvalidEvaluation);
        }
        1.
    } else {
        // Counts are bounded integers; ratio is checked again by compare_h.
        #[allow(clippy::cast_precision_loss)]
        {
            evolved_total as f64 / original_total as f64
        }
    };
    compare_h(
        &original.evidence.final_anchor,
        &evolved.evidence.final_anchor,
        &inc_health,
        &cand_health,
        &original.evidence.request.per_run_budget,
        policy,
    )
}
