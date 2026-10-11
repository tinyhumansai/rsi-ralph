//! Matches exact host attempts without copying duplicate or held-out labels.
use super::super::types::Entry;
use super::{
    ProposalLabel, ProposalOutcomeDataset, ProposalOutcomeRow, ProposalSelection, ProposalStage,
};
use crate::{
    Archive, Error, EvalDecision, EvalStage, GenerationAudit, NormalizedEvaluationEvent,
    ReasonCode, Result,
};
/// Joins only this journal occurrence to its exact archive attempt and frozen policy.
pub(super) fn project(
    entry: &Entry,
    generation: Option<GenerationAudit>,
    archive: &Archive,
) -> Result<ProposalOutcomeDataset> {
    if archive.package(&entry.incumbent_id).is_none() {
        return Err(Error::InvalidRecord);
    }
    let Some(prepared) = entry.prepared.as_ref() else {
        return Ok(build_dataset(
            entry,
            generation,
            entry.generation_mode.ok_or(Error::InvalidRecord)?,
            Vec::new(),
            entry.generation_failed,
        ));
    };
    let events = archive.normalized_evaluations();
    let mut rows = Vec::new();
    for (index, proposal) in prepared.proposals.iter().enumerate() {
        let receipt = entry.outcome.candidates.get(index);
        let interrupted = entry.evaluating && index == entry.outcome.candidates.len();
        let ambiguous = interrupted && prepared.proposals[..index].contains(proposal);
        let duplicate = receipt.is_some_and(|r| r.reason == ReasonCode::Duplicate);
        if let Some(id) = receipt.and_then(|r| r.candidate_id.as_ref())
            && archive.package(id).is_none()
        {
            return Err(Error::InvalidRecord);
        }
        let matching: Vec<_> = events
            .iter()
            .copied()
            .filter(|e| {
                !duplicate
                    && (receipt.is_some() || (interrupted && !ambiguous))
                    && e.epoch == entry.epoch
                    && e.incumbent_id == entry.incumbent_id
                    && e.proposal == *proposal
                    && receipt.is_none_or(|r| r.candidate_id.as_ref() == Some(&e.candidate_id))
            })
            .collect();
        if let Some(receipt) = receipt
            && !duplicate
        {
            validate_receipt(receipt, entry, proposal, &events, &matching)?;
            validate_disposition(receipt, &matching, archive)?;
        }
        validate_events(entry, &matching, proposal.prediction.minimum_gain)?;
        let last = matching.last();
        let selection = if entry.selection_binding.is_some() {
            matching
                .iter()
                .find(|e| e.stage == "Selection")
                .map(|e| ProposalSelection {
                    delta: e.comparison.delta,
                    lower: e.comparison.lower,
                    upper: e.comparison.upper,
                    eligible: match e.comparison.decision {
                        EvalDecision::Accept { .. } => Some(true),
                        EvalDecision::Reject { .. } => Some(false),
                        EvalDecision::Inconclusive { .. } => None,
                    },
                })
        } else {
            None
        };
        let label = label(entry, index, selection.as_ref(), duplicate, interrupted);
        let origin = entry.proposal_sources.as_ref().and_then(|s| s.get(index));
        let candidate_id = receipt
            .and_then(|r| r.candidate_id.clone())
            .or_else(|| last.map(|e| e.candidate_id.clone()));
        let stage = match last.map(|e| e.stage.as_str()) {
            Some("Screen") => EvalStage::Screen.into(),
            Some("Selection") => EvalStage::Selection.into(),
            Some("Confirmation") => EvalStage::Confirmation.into(),
            _ if candidate_id.is_some() && !duplicate => ProposalStage::Prepared,
            _ => ProposalStage::Generated,
        };
        rows.push(ProposalOutcomeRow {
            proposal_index: index,
            occurrence: origin.map(|s| s.occurrence.clone()),
            formatter_run_id: origin.map(|s| s.run_id.clone()),
            ranked: index < prepared.ranked_items,
            predicted_gain: proposal.prediction.minimum_gain,
            prior: entry.priors.as_ref().and_then(|p| p[index]),
            stage,
            reason: receipt.map(|r| r.reason),
            candidate_id,
            label,
            selection,
            interrupted,
        });
    }
    Ok(build_dataset(entry, generation, prepared.mode, rows, true))
}
/// Projects the same provenance for successful, failed and incomplete generation.
fn build_dataset(
    entry: &Entry,
    generation: Option<GenerationAudit>,
    mode: crate::GenerationMode,
    rows: Vec<ProposalOutcomeRow>,
    generation_complete: bool,
) -> ProposalOutcomeDataset {
    ProposalOutcomeDataset {
        version: 1,
        epoch: entry.epoch,
        fingerprint: entry.fingerprint.clone(),
        mode,
        model: entry.model.clone(),
        generation_seed: entry.generation_seed,
        generation,
        meter: entry.meter.clone(),
        generation_complete,
        rows,
    }
}

/// Keeps missing and censored outcomes separate from resolved eligibility.
fn label(
    entry: &Entry,
    index: usize,
    selection: Option<&ProposalSelection>,
    duplicate: bool,
    interrupted: bool,
) -> ProposalLabel {
    if duplicate {
        ProposalLabel::Duplicate
    } else if let Some(selection) = selection {
        if selection.eligible.is_some() {
            ProposalLabel::Observed
        } else {
            ProposalLabel::Inconclusive
        }
    } else if interrupted {
        ProposalLabel::Interrupted
    } else if index < entry.outcome.candidates.len() {
        ProposalLabel::NotSelected
    } else if entry.outcome.complete {
        ProposalLabel::Censored
    } else {
        ProposalLabel::Pending
    }
}
/// Ensures observed stages used the host policy and private-suite assignment frozen at generation.
fn validate_events(entry: &Entry, events: &[&NormalizedEvaluationEvent], gain: f64) -> Result<()> {
    if events
        .windows(2)
        .any(|p| p[0].candidate_id != p[1].candidate_id)
    {
        return Err(Error::InvalidRecord);
    }
    if let Some(binding) = &entry.selection_binding {
        let mut policy = binding.policy.clone();
        policy.margin = policy.margin.max(gain);
        if events.iter().any(|e| {
            e.policy != policy || e.suite_ids != binding.suites || e.budget != binding.budget
        }) {
            return Err(Error::InvalidRecord);
        }
    }
    Ok(())
}

/// Refuses contradictory candidate bindings instead of treating their labels as missing.
fn validate_receipt(
    receipt: &super::super::EpochCandidateOutcome,
    entry: &Entry,
    proposal: &crate::Proposal,
    events: &[&NormalizedEvaluationEvent],
    matching: &[&NormalizedEvaluationEvent],
) -> Result<()> {
    let required = match receipt.reason {
        ReasonCode::Accepted | ReasonCode::ApprovalDenied | ReasonCode::NotReplicated => 3,
        ReasonCode::Inconclusive => 2,
        ReasonCode::NoGain => 1,
        _ => 0,
    };
    if matching.len() < required
        || events.iter().any(|e| {
            receipt.candidate_id.as_ref() == Some(&e.candidate_id)
                && (e.epoch != entry.epoch
                    || e.incumbent_id != entry.incumbent_id
                    || e.proposal != *proposal)
        })
    {
        return Err(Error::InvalidRecord);
    }
    Ok(())
}

/// Validates final reasons against observed decisions and exact human approval.
fn validate_disposition(
    receipt: &super::super::EpochCandidateOutcome,
    events: &[&NormalizedEvaluationEvent],
    archive: &Archive,
) -> Result<()> {
    let approval = archive
        .normalized_approvals()
        .into_iter()
        .find(|a| receipt.candidate_id.as_ref() == Some(&a.candidate_id));
    let last = events.last();
    let valid = match receipt.reason {
        ReasonCode::Accepted | ReasonCode::ApprovalDenied => {
            events.len() == 3
                && events[1..]
                    .iter()
                    .all(|e| matches!(e.comparison.decision, EvalDecision::Accept { .. }))
                && approval.is_some_and(|a| {
                    a.approved == (receipt.reason == ReasonCode::Accepted)
                        && last.is_some_and(|e| a.comparison == e.comparison)
                })
        }
        ReasonCode::NoGain => {
            approval.is_none()
                && last.is_some_and(|e| {
                    e.stage != "Confirmation"
                        && matches!(e.comparison.decision, EvalDecision::Reject { .. })
                })
        }
        ReasonCode::Inconclusive => {
            approval.is_none()
                && last.is_some_and(|e| {
                    e.stage == "Selection"
                        && matches!(e.comparison.decision, EvalDecision::Inconclusive { .. })
                })
        }
        ReasonCode::NotReplicated => {
            approval.is_none()
                && last.is_some_and(|e| {
                    e.stage == "Confirmation"
                        && archive.proposals().iter().any(|p| {
                            p.epoch == e.epoch
                                && p.patch == e.proposal.patch
                                && p.rationale
                                    == format!("{}: {}", e.proposal.title, e.proposal.mechanism)
                                && p.transcript_refs == e.proposal.transcript_refs
                                && p.reason == "NotReplicated"
                        })
                })
        }
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidRecord)
    }
}
