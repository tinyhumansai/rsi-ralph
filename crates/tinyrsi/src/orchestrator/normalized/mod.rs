//! Human-gated normalized proposal evaluation with replayable bootstrap evidence.
//!
//! The legacy raw-quality runner remains separate. This route validates a
//! `normalized_h` prediction, binds immutable private suites and policy, persists
//! every completed paired comparison, and asks a human about H evidence itself.

use super::{apply_at, event, prefilter_for, prepare};
use crate::archive::{
    NormalizedApprovalEvent, NormalizedDeclarationEvent, NormalizedEvaluationEvent,
};
use crate::{
    Archive, CandidateState, Error, EvalDecision, EvalRun, EvalStage, Proposal,
    ProposalDeclaration, ReasonCode, Result, ScoreComparison, compare_h, ensure_independent,
};
use crate::{ScoringPolicy, ScoringPolicySpec};
use std::collections::{BTreeMap, BTreeSet};
use tinyhivemind_lang::lineage::{Direction, ObjectiveScore, Verdict, VersionMetadata};

mod types;
pub use types::*;

/// Evaluate normalized H through frozen screening, selection, and confirmation.
///
/// Every accepted design has independently confirmed paired bootstrap evidence
/// and a durable explicit human approval. No raw comparison is substituted for
/// H. Returned activation plans must be executed by the host at a safe boundary.
///
/// # Errors
/// Returns invalid configuration or archive failures. Host execution and approval
/// failures become durable `HostFailure` outcomes without advancing the head.
pub fn run_proposal_h(
    archive: &mut Archive,
    epoch: &HEpoch,
    proposal: &Proposal,
    evaluator: &mut impl HEvaluator,
    approval: &mut impl HApprovalGate,
) -> Result<HCandidateOutcome> {
    validate_epoch(epoch)?;
    let head = archive.head().ok_or(Error::InvalidRecord)?.clone();
    let incumbent = CandidateState {
        package: archive
            .package(&head.id)
            .ok_or(Error::InvalidRecord)?
            .clone(),
        memory_watermarks: head.memory_watermarks.clone(),
    };
    if let Some(reason) = prefilter_for(archive, &epoch.epoch, proposal, "normalized_h", None) {
        return stopped(archive, epoch, proposal, reason, None, None);
    }
    if let Some(id) = archive.normalized_existing_candidate(&head.id, proposal) {
        return stopped(
            archive,
            epoch,
            proposal,
            ReasonCode::Duplicate,
            Some(id),
            None,
        );
    }
    let mut spec = epoch.policy.clone();
    spec.margin = spec.margin.max(proposal.prediction.minimum_gain);
    let policy = spec.build()?;
    preregister(archive, epoch, proposal, &head.id)?;
    let (candidate, id) = match prepare(&incumbent, proposal, evaluator) {
        Ok(prepared) => prepared,
        Err(reason) => return stopped(archive, epoch, proposal, reason, None, None),
    };
    if archive.package(&id).is_some() {
        return stopped(
            archive,
            epoch,
            proposal,
            ReasonCode::Duplicate,
            Some(id),
            None,
        );
    }
    archive.prepare(
        candidate.package.clone(),
        candidate.memory_watermarks.clone(),
    )?;
    let mut flow = Flow {
        archive,
        epoch,
        proposal,
        incumbent,
        parent: head.id,
        candidate,
        id,
        spec,
        policy,
        last: None,
        selection: vec![],
    };
    for stage in [
        EvalStage::Screen,
        EvalStage::Selection,
        EvalStage::Confirmation,
    ] {
        let Some(pair) = flow.measure(stage, evaluator)? else {
            return flow.stop(ReasonCode::HostFailure);
        };
        if stage == EvalStage::Confirmation
            && ensure_independent(&flow.selection, &pair.pair.candidate).is_err()
        {
            return flow.stop(ReasonCode::NotReplicated);
        }
        let comparison = flow.last.as_ref().ok_or(Error::InvalidEvaluation)?;
        let reason = match &comparison.decision {
            EvalDecision::Reject { .. } => Some(if stage == EvalStage::Confirmation {
                ReasonCode::NotReplicated
            } else {
                ReasonCode::NoGain
            }),
            EvalDecision::Inconclusive { .. } if stage != EvalStage::Screen => {
                Some(if stage == EvalStage::Confirmation {
                    ReasonCode::NotReplicated
                } else {
                    ReasonCode::Inconclusive
                })
            }
            _ => None,
        };
        if let Some(reason) = reason {
            return flow.stop(reason);
        }
        if stage == EvalStage::Confirmation {
            return flow.finish(approval);
        }
        flow.selection.extend(pair.pair.candidate);
    }
    Err(Error::InvalidEvaluation)
}

fn preregister(
    archive: &mut Archive,
    epoch: &HEpoch,
    proposal: &Proposal,
    parent: &str,
) -> Result<()> {
    archive.declare_proposal(ProposalDeclaration {
        epoch: epoch.epoch.id,
        proposal: proposal.clone(),
    })?;
    archive.declare_normalized_proposal(NormalizedDeclarationEvent {
        epoch: epoch.epoch.id,
        proposal: proposal.clone(),
        incumbent_id: parent.to_owned(),
        policy: epoch.policy.clone(),
        budget: epoch.epoch.budget.clone(),
        suite_ids: epoch.suite_ids.clone(),
    })?;
    Ok(())
}

fn validate_epoch(epoch: &HEpoch) -> Result<()> {
    epoch.policy.build()?;
    let values: BTreeSet<_> = epoch.suite_ids.values().collect();
    if epoch.epoch.budget.max_tokens == 0
        || epoch.suite_ids.len() != 3
        || values.len() != 3
        || ["Screen", "Selection", "Confirmation"].iter().any(|stage| {
            epoch
                .suite_ids
                .get(*stage)
                .is_none_or(|id| id.trim().is_empty())
        })
    {
        return Err(Error::InvalidEvaluation);
    }
    Ok(())
}

struct Flow<'a> {
    archive: &'a mut Archive,
    epoch: &'a HEpoch,
    proposal: &'a Proposal,
    incumbent: CandidateState,
    parent: String,
    candidate: CandidateState,
    id: String,
    spec: ScoringPolicySpec,
    policy: ScoringPolicy,
    last: Option<ScoreComparison>,
    selection: Vec<EvalRun>,
}

impl Flow<'_> {
    fn measure(
        &mut self,
        stage: EvalStage,
        evaluator: &mut impl HEvaluator,
    ) -> Result<Option<HEvalPair>> {
        let Ok(pair) = evaluator.evaluate_h(
            &self.incumbent,
            &self.candidate,
            stage,
            &self.epoch.epoch.budget,
        ) else {
            return Ok(None);
        };
        let stage_name = format!("{stage:?}");
        if self.epoch.suite_ids.get(&stage_name) != Some(&pair.suite_id) {
            return Ok(None);
        }
        let Ok(comparison) = compare_h(
            &pair.pair.incumbent,
            &pair.pair.candidate,
            &pair.incumbent_health,
            &pair.candidate_health,
            &self.epoch.epoch.budget,
            &self.policy,
        ) else {
            return Ok(None);
        };
        self.archive
            .record_normalized_evaluation(NormalizedEvaluationEvent {
                candidate_id: self.id.clone(),
                incumbent_id: self.parent.clone(),
                epoch: self.epoch.epoch.id,
                proposal: self.proposal.clone(),
                stage: stage_name,
                suite_id: pair.suite_id.clone(),
                suite_ids: self.epoch.suite_ids.clone(),
                incumbent: pair.pair.incumbent.clone(),
                candidate: pair.pair.candidate.clone(),
                incumbent_health: pair.incumbent_health.clone(),
                candidate_health: pair.candidate_health.clone(),
                budget: self.epoch.epoch.budget.clone(),
                policy: self.spec.clone(),
                comparison: comparison.clone(),
            })?;
        self.last = Some(comparison);
        Ok(Some(pair))
    }

    fn metadata(&self, verdict: Verdict) -> Result<VersionMetadata> {
        let comparison = self.last.as_ref().ok_or(Error::InvalidEvaluation)?;
        Ok(VersionMetadata {
            parent: Some(self.parent.clone()),
            patch: self.proposal.patch.clone(),
            rationale: format!("{}: {}", self.proposal.title, self.proposal.mechanism),
            transcript_refs: self.proposal.transcript_refs.clone(),
            scores: BTreeMap::from([(
                "normalized_h".into(),
                ObjectiveScore {
                    value: comparison.candidate.value,
                    direction: Direction::Maximize,
                },
            )]),
            verdict,
            memory_watermarks: self.candidate.memory_watermarks.clone(),
        })
    }

    fn stop(self, reason: ReasonCode) -> Result<HCandidateOutcome> {
        if self.last.is_some() {
            let metadata = self.metadata(Verdict::Rejected)?;
            self.archive.conclude(
                self.candidate.package.clone(),
                metadata,
                event(&self.epoch.epoch, self.proposal, reason),
            )?;
            Ok(HCandidateOutcome {
                reason,
                candidate_id: Some(self.id),
                comparison: self.last,
                apply_at: None,
            })
        } else {
            stopped(
                self.archive,
                self.epoch,
                self.proposal,
                reason,
                Some(self.id),
                None,
            )
        }
    }

    fn finish(self, approval: &mut impl HApprovalGate) -> Result<HCandidateOutcome> {
        let comparison = self.last.as_ref().ok_or(Error::InvalidEvaluation)?;
        let Ok(approved) = approval.approve_h(&self.id, self.proposal, comparison) else {
            return self.stop(ReasonCode::HostFailure);
        };
        self.archive
            .record_normalized_approval(NormalizedApprovalEvent {
                candidate_id: self.id.clone(),
                comparison: comparison.clone(),
                approved,
            })?;
        let reason = if approved {
            ReasonCode::Accepted
        } else {
            ReasonCode::ApprovalDenied
        };
        let metadata = self.metadata(if approved {
            Verdict::Accepted
        } else {
            Verdict::Rejected
        })?;
        self.archive.conclude(
            self.candidate.package.clone(),
            metadata,
            event(&self.epoch.epoch, self.proposal, reason),
        )?;
        Ok(HCandidateOutcome {
            reason,
            candidate_id: Some(self.id),
            comparison: self.last,
            apply_at: approved.then(|| apply_at(self.proposal.patch.class)),
        })
    }
}

fn stopped(
    archive: &mut Archive,
    epoch: &HEpoch,
    proposal: &Proposal,
    reason: ReasonCode,
    id: Option<String>,
    comparison: Option<ScoreComparison>,
) -> Result<HCandidateOutcome> {
    archive.record_proposal(event(&epoch.epoch, proposal, reason))?;
    Ok(HCandidateOutcome {
        reason,
        candidate_id: id,
        comparison,
        apply_at: None,
    })
}

#[cfg(test)]
mod test;
