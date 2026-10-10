//! Restart-safe normalized proposal generation and separate synchronous evaluation.
mod generation;
mod journal;
mod types;
use crate::{
    Archive, DurableRetreatMemory, Error, HApprovalGate, HEpoch, HEvaluator, ReasonCode, Result,
    RetreatMemoryOutcome, RetreatOutcomeStatus, run_proposal_h,
};
pub use generation::{RetreatEpochInput, normalized_retreat_request, prepare_retreat_epoch};
pub use journal::EpochJournal;
pub use types::*;
/// Evaluates already-persisted ranked proposals outside the async generation executor.
///
/// Only the supplied explicit human gate can approve promotion. Live activation
/// remains a separate `DeploymentStore` operation. Interrupted evaluations fail
/// closed rather than repeating uncertain private-suite or approval effects.
/// # Errors
/// Returns invalid epoch bindings, archive errors or an interrupted-host error.
pub fn evaluate_retreat_epoch(
    journal: &mut EpochJournal,
    archive: &mut Archive,
    epoch: &HEpoch,
    evaluator: &mut impl HEvaluator,
    approval: &mut impl HApprovalGate,
    memory: &DurableRetreatMemory,
) -> Result<RetreatEpochOutcome> {
    let mut entry = journal
        .state
        .entries
        .get(&epoch.epoch.id)
        .cloned()
        .ok_or(Error::InvalidProposal)?;
    let prepared = entry.prepared.clone().ok_or(Error::HostFailed)?;
    if entry.evaluating {
        return Err(Error::HostFailed);
    }
    for old in &entry.outcome.candidates {
        remember(memory, prepared.mode, prepared.epoch, old)?;
    }
    if entry.outcome.complete {
        return Ok(entry.outcome);
    }
    if archive.head().is_none_or(|h| h.id != prepared.incumbent_id) {
        return Err(Error::InvalidRecord);
    }
    if generation::policy_fingerprint(epoch)? != prepared.policy_fingerprint {
        return Err(Error::InvalidProposal);
    }
    for index in entry.outcome.candidates.len()..prepared.proposals.len() {
        entry.evaluating = true;
        journal.commit_entry(entry.clone())?;
        let outcome = run_proposal_h(
            archive,
            epoch,
            &prepared.proposals[index],
            evaluator,
            approval,
        )?;
        let receipt = EpochCandidateOutcome {
            proposal_index: index,
            reason: outcome.reason,
            candidate_id: outcome.candidate_id,
        };
        entry.outcome.candidates.push(receipt.clone());
        entry.evaluating = false;
        entry.outcome.complete =
            receipt.reason == ReasonCode::Accepted || index + 1 == prepared.proposals.len();
        journal.commit_entry(entry.clone())?;
        remember(memory, prepared.mode, prepared.epoch, &receipt)?;
        if entry.outcome.complete {
            break;
        }
    }
    if prepared.proposals.is_empty() {
        entry.outcome.complete = true;
        journal.commit_entry(entry.clone())?;
    }
    Ok(entry.outcome)
}
fn remember(
    memory: &DurableRetreatMemory,
    mode: GenerationMode,
    epoch: u64,
    outcome: &EpochCandidateOutcome,
) -> Result<()> {
    if mode == GenerationMode::Retreat
        && let Some(id) = &outcome.candidate_id
    {
        memory.record_outcome(RetreatMemoryOutcome {
            epoch,
            candidate_id: id.clone(),
            status: if outcome.reason == ReasonCode::Accepted {
                RetreatOutcomeStatus::Approved
            } else {
                RetreatOutcomeStatus::Rejected
            },
        })?;
    }
    Ok(())
}
#[cfg(test)]
mod test;
