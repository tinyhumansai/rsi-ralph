//! Recording a hive epoch on the loop trace.
//!
//! The script loop writes its own trace as it runs. A hive epoch is driven by
//! the host one call at a time, so these two functions turn what those calls
//! return into the same `LoopEvent`s, and `apps/rsi-viz` shows a hive epoch
//! exactly as it shows a script run. The host decides when to record and
//! still emits `epoch_started`, `observed` and the run boundaries itself,
//! because only it knows where its run begins and what it observed.
use super::{PreparedRetreatEpoch, RetreatEpochOutcome};
use crate::{Archive, EvalDecision, LoopEvent, ReasonCode, Result, ScoreDecision, TraceSink};

/// The trace id of the `index`-th proposal of `epoch`, as the script loop
/// names its own (`p<epoch>-<index>`).
fn proposal_id(epoch: u64, index: usize) -> String {
    format!("p{epoch}-{index}")
}

/// Records one `ideated` event per prepared proposal, in rank order, under
/// the trace's `epoch` label (the host's own epoch numbering, which need not
/// be the hive epoch id).
///
/// Hive candidate ids exist only after a proposal is formalized during
/// evaluation, so `candidate` is `None` here and `rationale` / `hypothesis`
/// carry the proposal's mechanism and title.
///
/// # Errors
///
/// Returns [`crate::Error::TraceIo`] when the sink cannot record.
pub fn record_ideated(
    trace: &mut dyn TraceSink,
    epoch: u64,
    prepared: &PreparedRetreatEpoch,
) -> Result<()> {
    for (index, proposal) in prepared.proposals.iter().enumerate() {
        trace.record(LoopEvent::Ideated {
            epoch,
            proposal_id: proposal_id(epoch, index),
            parent: prepared.incumbent_id.clone(),
            candidate: None,
            rationale: proposal.mechanism.clone(),
            hypothesis: proposal.title.clone(),
        })?;
    }
    Ok(())
}

fn decision(d: &EvalDecision) -> ScoreDecision {
    match d {
        EvalDecision::Accept { .. } => ScoreDecision::Accept,
        EvalDecision::Reject { .. } => ScoreDecision::Reject,
        EvalDecision::Inconclusive { .. } => ScoreDecision::Inconclusive,
    }
}

/// Records how each evaluated proposal fared, then `epoch_finished`, under
/// the trace's `epoch` label.
///
/// For every candidate in `outcome`: `measured` (its normalized H estimate,
/// when the archive holds an evaluation of it), `scored` (accepted only when
/// the human approved it; an inconclusive evaluation keeps its own decision;
/// anything else is a rejection, with a zero interval when nothing was
/// evaluated), and `improved` from `incumbent` when it was accepted. Hive
/// candidates have no per-seed script metric, so `executed` is not emitted.
///
/// # Errors
///
/// Returns [`crate::Error::TraceIo`] when the sink cannot record.
pub fn record_outcome(
    trace: &mut dyn TraceSink,
    epoch: u64,
    archive: &Archive,
    incumbent: &str,
    outcome: &RetreatEpochOutcome,
) -> Result<()> {
    let evaluations = archive.normalized_evaluations();
    for c in &outcome.candidates {
        let candidate = c
            .candidate_id
            .clone()
            .unwrap_or_else(|| proposal_id(epoch, c.proposal_index));
        let evaluation = evaluations
            .iter()
            .rev()
            .find(|e| e.candidate_id == candidate);
        if let Some(e) = evaluation {
            trace.record(LoopEvent::Measured {
                epoch,
                candidate: candidate.clone(),
                mean: Some(e.comparison.candidate.value),
                failures: 0,
                seeds: u32::try_from(e.candidate.len()).unwrap_or(u32::MAX),
            })?;
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
        })?;
        if let (true, Some(e)) = (accepted, evaluation) {
            trace.record(LoopEvent::Improved {
                epoch,
                from: incumbent.to_owned(),
                to: candidate,
                metric: e.comparison.candidate.value,
            })?;
        }
    }
    trace.record(LoopEvent::EpochFinished {
        epoch,
        incumbent: archive
            .head()
            .map_or_else(|| incumbent.to_owned(), |h| h.id.clone()),
        incumbent_metric: None,
    })?;
    Ok(())
}
