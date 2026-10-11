//! Recording a hive epoch on the loop trace.
//!
//! The script loop writes its own trace as it runs. A hive epoch is driven by
//! the host one call at a time, so [`record_epoch`] turns what those calls
//! return into the same `LoopEvent`s, and `apps/rsi-viz` shows a hive epoch
//! exactly as it shows a script run. It records after evaluation, because a
//! hive candidate id exists only once its proposal is formalized, and the
//! visualizer pairs ideas with candidates through `ideated.candidate`. The host
//! still emits `epoch_started`, `observed` and the run boundaries itself,
//! because only it knows where its run begins and what it observed.
use super::{PreparedRetreatEpoch, RetreatEpochOutcome};
use crate::{
    Archive, EvalDecision, IntervalUnits, LoopEvent, ReasonCode, Result, ScoreDecision, TraceSink,
};

/// The trace id of the `index`-th proposal of `epoch`, as the script loop
/// names its own (`p<epoch>-<index>`).
fn proposal_id(epoch: u64, index: usize) -> String {
    format!("p{epoch}-{index}")
}

fn decision(d: &EvalDecision) -> ScoreDecision {
    match d {
        EvalDecision::Accept { .. } => ScoreDecision::Accept,
        EvalDecision::Reject { .. } => ScoreDecision::Reject,
        EvalDecision::Inconclusive { .. } => ScoreDecision::Inconclusive,
    }
}

/// Records one evaluated hive epoch under the trace's `epoch` label (the
/// host's own numbering, which need not be the hive epoch id).
///
/// - `ideated` for every prepared proposal, in rank order, naming the
///   candidate it was formalized into, or none when it never was or when it
///   duplicated a patch already evaluated.
/// - For each candidate this epoch actually evaluated: `measured` (its
///   normalized H estimate), `scored` (accepted only when the human approved
///   it; an inconclusive evaluation keeps its own decision; anything else is a
///   rejection), and `improved` from the prepared incumbent when accepted.
///   A candidate that failed runs its incumbent completed (`HARMFUL`) is
///   measured from its `CandidateFailureEvent`: no mean, its failed runs as
///   `failures` and the runs attempted as `seeds`.
///   Evaluations are matched on this epoch, incumbent, proposal and candidate,
///   so an earlier epoch's evidence is never replayed. A proposal that was
///   never formalized, or that duplicated an evaluated patch, ran nothing and
///   gets no `measured` or `scored`.
/// - `epoch_finished` with the accepted candidate's H, or else the
///   incumbent's H on this epoch's private draw, or none when nothing ran.
///
/// Hive candidates have no per-seed script metric, so `executed` is not
/// emitted.
///
/// # Errors
///
/// Returns [`crate::Error::TraceIo`] when the sink cannot record.
pub fn record_epoch(
    trace: &mut dyn TraceSink,
    epoch: u64,
    archive: &Archive,
    prepared: &PreparedRetreatEpoch,
    outcome: &RetreatEpochOutcome,
) -> Result<()> {
    let candidate_of = |index: usize| {
        outcome
            .candidates
            .iter()
            .find(|c| c.proposal_index == index)
            // A duplicate ran nothing; linking it to the earlier candidate
            // would show that candidate's measurement as its own.
            .filter(|c| c.reason != ReasonCode::Duplicate)
            .and_then(|c| c.candidate_id.clone())
    };
    for (index, proposal) in prepared.proposals.iter().enumerate() {
        trace.record(LoopEvent::Ideated {
            epoch,
            proposal_id: proposal_id(epoch, index),
            parent: prepared.incumbent_id.clone(),
            candidate: candidate_of(index),
            rationale: proposal.mechanism.clone(),
            hypothesis: proposal.title.clone(),
            approach: None,
        })?;
    }
    let evaluations = archive.normalized_evaluations();
    let failures = archive.candidate_failures();
    let mut incumbent_metric = None;
    for c in &outcome.candidates {
        let (Some(candidate), Some(proposal)) = (
            c.candidate_id.as_ref(),
            prepared.proposals.get(c.proposal_index),
        ) else {
            continue;
        };
        if c.reason == ReasonCode::Duplicate {
            continue;
        }
        let evaluation = evaluations.iter().rev().find(|e| {
            e.epoch == prepared.epoch
                && e.incumbent_id == prepared.incumbent_id
                && e.proposal == *proposal
                && e.candidate_id == *candidate
        });
        let failure = failures
            .iter()
            .rev()
            .find(|f| f.epoch == prepared.epoch && f.candidate_id == *candidate);
        if let Some(f) = failure {
            trace.record(LoopEvent::Measured {
                epoch,
                candidate: candidate.clone(),
                mean: None,
                failures: f.failed_runs,
                seeds: f.runs,
            })?;
        } else if let Some(e) = evaluation {
            trace.record(LoopEvent::Measured {
                epoch,
                candidate: candidate.clone(),
                mean: Some(e.comparison.candidate.value),
                failures: 0,
                seeds: u32::try_from(e.candidate.len()).unwrap_or(u32::MAX),
            })?;
        }
        if let Some(e) = evaluation {
            incumbent_metric = incumbent_metric.or(Some(e.comparison.incumbent.value));
        }
        let accepted = c.reason == ReasonCode::Accepted;
        trace.record(LoopEvent::Scored {
            epoch,
            candidate: candidate.clone(),
            decision: match (accepted, evaluation) {
                (true, _) => ScoreDecision::Accept,
                (false, Some(e)) if c.reason == ReasonCode::Inconclusive => {
                    decision(&e.comparison.decision)
                }
                _ => ScoreDecision::Reject,
            },
            estimate: evaluation.map_or(0.0, |e| e.comparison.delta),
            lower: evaluation.map_or(0.0, |e| e.comparison.lower),
            upper: evaluation.map_or(0.0, |e| e.comparison.upper),
            // A hive run's goal metric is normalized H itself.
            units: IntervalUnits::Metric,
        })?;
        if let (true, Some(e)) = (accepted, evaluation) {
            trace.record(LoopEvent::Improved {
                epoch,
                from: prepared.incumbent_id.clone(),
                to: candidate.clone(),
                metric: e.comparison.candidate.value,
            })?;
            incumbent_metric = Some(e.comparison.candidate.value);
        }
    }
    trace.record(LoopEvent::EpochFinished {
        epoch,
        incumbent: archive
            .head()
            .map_or_else(|| prepared.incumbent_id.clone(), |h| h.id.clone()),
        incumbent_metric,
    })?;
    Ok(())
}
