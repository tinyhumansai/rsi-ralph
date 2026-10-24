//! Actual callback execution and paired trial bootstrap summaries.

use super::ledger::{compare_arms, failed, sum, validate_trial};
use super::{
    AblationEstimate, EvidenceSource, ExperimentAblation, ExperimentArm, ExperimentExecutor,
    ExperimentProtocol, ExperimentReport, ExperimentUncertainty, ExperimentVerdict, TrialFailure,
    TrialRequest, TrialSeeds, TrialSummary, ValidTrial,
};
use crate::{Error, EvalDecision, Result, ScoringPolicy};

/// Execute all frozen arm/ablation assignments and produce a redacted report.
///
/// Every callback starts from the same hive. Arm order alternates across trial
/// draws. Host errors and invalid ledgers count all requested attempts and both
/// complete token reservations, and force an inconclusive live conclusion.
/// Scripted callbacks always produce `ProtocolOnly`, regardless of their scores.
///
/// # Errors
/// Returns [`Error::InvalidEvaluation`] if validated policy reconstruction or
/// report arithmetic fails. Callback failures are retained in the report.
pub fn run_experiment(
    protocol: &ExperimentProtocol,
    executor: &mut impl ExperimentExecutor,
) -> Result<ExperimentReport> {
    let policy = protocol.spec.scoring.build()?;
    let mut trials = Vec::new();
    let mut ablations = Vec::new();
    let mut shape = None;
    for ablation in [
        ExperimentAblation::Full,
        ExperimentAblation::MemoryOff,
        ExperimentAblation::RetreatOff,
    ] {
        let mut deltas = Vec::new();
        let mut anchors_accepted = true;
        for (index, seeds) in protocol.spec.trials.iter().enumerate() {
            let mut arms = [ExperimentArm::Original, ExperimentArm::Evolved];
            if index % 2 == 1 {
                arms.reverse();
            }
            let mut original = None;
            let mut evolved = None;
            for arm in arms {
                let request = assignment(protocol, arm, ablation, seeds.clone());
                let result = executor.execute_trial(&request);
                let failure = if result.is_err() {
                    TrialFailure::HostError
                } else {
                    TrialFailure::InvalidEvidence
                };
                let valid = result.and_then(|e| validate_trial(e, &request, &policy, &mut shape));
                match valid {
                    Ok(trial) => match arm {
                        ExperimentArm::Original => original = Some(trial),
                        ExperimentArm::Evolved => evolved = Some(trial),
                    },
                    Err(_) => trials.push(failed(&request, failure)),
                }
            }
            add_pair(
                &mut trials,
                &mut deltas,
                &mut anchors_accepted,
                original,
                evolved,
                &policy,
            )?;
        }
        ablations.push(estimate(ablation, &deltas, anchors_accepted, protocol)?);
    }
    let complete = trials.iter().all(|t| t.verified);
    let full = &ablations[0];
    let verdict = if protocol.spec.source == EvidenceSource::Scripted {
        ExperimentVerdict::ProtocolOnly
    } else if !complete || full.pairs < 3 {
        ExperimentVerdict::Inconclusive
    } else if full.lower > protocol.spec.efficiency_margin && full.anchors_accepted {
        ExperimentVerdict::SupportedImprovement
    } else if full.upper <= protocol.spec.efficiency_margin {
        ExperimentVerdict::Negative
    } else {
        ExperimentVerdict::Inconclusive
    };
    Ok(ExperimentReport {
        uncertainty: ExperimentUncertainty::PairedTrialPercentileBootstrap,
        bootstrap_resamples: protocol.spec.scoring.resamples,
        efficiency_margin: protocol.spec.efficiency_margin,
        generation_cap_per_trial: protocol.spec.generation_cap,
        evaluation_cap_per_trial: protocol.spec.evaluation_cap,
        protocol_id: protocol.fingerprint.clone(),
        source: protocol.spec.source,
        attempts: sum(trials.iter().map(|t| u64::from(t.attempts)))?,
        generation_tokens: sum(trials.iter().map(|t| t.generation_tokens))?,
        evaluation_tokens: sum(trials.iter().map(|t| t.evaluation_tokens))?,
        trials,
        ablations,
        verdict,
    })
}

fn assignment(
    protocol: &ExperimentProtocol,
    arm: ExperimentArm,
    ablation: ExperimentAblation,
    seeds: TrialSeeds,
) -> TrialRequest {
    TrialRequest {
        protocol_id: protocol.fingerprint.clone(),
        arm,
        ablation,
        seeds,
        config_id: match arm {
            ExperimentArm::Original => protocol.spec.original_config_id.clone(),
            ExperimentArm::Evolved => protocol.spec.evolved_config_id.clone(),
        },
        initial_hive_id: protocol.spec.initial_hive_id.clone(),
        suites: protocol.spec.suites.clone(),
        attempts_per_trial: protocol.spec.attempts_per_trial,
        generation_cap: protocol.spec.generation_cap,
        evaluation_cap: protocol.spec.evaluation_cap,
        per_run_budget: protocol.spec.per_run_budget.clone(),
    }
}

fn add_pair(
    trials: &mut Vec<TrialSummary>,
    deltas: &mut Vec<f64>,
    accepted: &mut bool,
    original: Option<ValidTrial>,
    evolved: Option<ValidTrial>,
    policy: &ScoringPolicy,
) -> Result<()> {
    match (original, evolved) {
        (Some(mut original), Some(mut evolved)) => {
            if let Ok(comparison) = compare_arms(&original, &evolved, policy) {
                let efficiency = |t: &ValidTrial| -> Result<f64> {
                    let tokens = t
                        .summary
                        .generation_tokens
                        .checked_add(t.summary.evaluation_tokens)
                        .ok_or(Error::InvalidEvaluation)?;
                    // Positive bounded total was validated before this division.
                    #[allow(clippy::cast_precision_loss)]
                    let value = t.comparison.delta / tokens as f64 * 1000.;
                    if value.is_finite() {
                        Ok(value)
                    } else {
                        Err(Error::InvalidEvaluation)
                    }
                };
                let delta = efficiency(&evolved)? - efficiency(&original)?;
                if !delta.is_finite() {
                    return Err(Error::InvalidEvaluation);
                }
                deltas.push(delta);
                *accepted &= evolved.summary.accepted > 0
                    && evolved.summary.anchor_accepted
                    && matches!(comparison.decision, EvalDecision::Accept { .. });
            } else {
                for summary in [&mut original.summary, &mut evolved.summary] {
                    summary.verified = false;
                    summary.failure = Some(TrialFailure::InvalidEvidence);
                }
                *accepted = false;
            }
            trials.push(original.summary);
            trials.push(evolved.summary);
        }
        (Some(trial), None) | (None, Some(trial)) => {
            trials.push(trial.summary);
            *accepted = false;
        }
        (None, None) => {
            *accepted = false;
        }
    }
    Ok(())
}

fn estimate(
    ablation: ExperimentAblation,
    deltas: &[f64],
    anchors_accepted: bool,
    protocol: &ExperimentProtocol,
) -> Result<AblationEstimate> {
    let pairs = u32::try_from(deltas.len()).map_err(|_| Error::InvalidEvaluation)?;
    if deltas.is_empty() {
        return Ok(AblationEstimate {
            ablation,
            pairs,
            efficiency_delta: 0.,
            lower: 0.,
            upper: 0.,
            anchors_accepted: false,
        });
    }
    let mean = |values: &[f64]| -> Result<f64> {
        let count = u32::try_from(values.len()).map_err(|_| Error::InvalidEvaluation)?;
        let value = values.iter().sum::<f64>() / f64::from(count);
        if value.is_finite() {
            Ok(value)
        } else {
            Err(Error::InvalidEvaluation)
        }
    };
    let efficiency_delta = mean(deltas)?;
    let salt = match ablation {
        ExperimentAblation::Full => 1,
        ExperimentAblation::MemoryOff => 2,
        ExperimentAblation::RetreatOff => 3,
    };
    let mut state = protocol.spec.scoring.bootstrap_seed ^ salt;
    let mut draws = Vec::new();
    for _ in 0..protocol.spec.scoring.resamples {
        let mut sample = Vec::with_capacity(deltas.len());
        for _ in deltas {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let index = usize::try_from((state >> 32) % u64::from(pairs))
                .map_err(|_| Error::InvalidEvaluation)?;
            sample.push(deltas[index]);
        }
        draws.push(mean(&sample)?);
    }
    draws.sort_by(f64::total_cmp);
    let last = draws.len() - 1;
    Ok(AblationEstimate {
        ablation,
        pairs,
        efficiency_delta,
        lower: draws[last * 25 / 1000],
        upper: draws[last * 975 / 1000],
        anchors_accepted,
    })
}
