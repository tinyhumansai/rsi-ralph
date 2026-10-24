//! Guarded proposal funnel with paired shadow evaluation and human promotion.
//!
//! The host owns private tasks, memory forks and activation. This module selects
//! a durable design and returns its activation granularity; it never writes to
//! a live coordinator or silently bypasses approval.
mod normalized;
mod types;
use crate::{
    Archive, Comparison, Error, EvalDecision, EvalPolicy, EvalRun, PatchClass, ProposalEvent,
    Result, compare, ensure_independent,
};
use crate::{CandidateState, EvalStage, Proposal, ReasonCode};
pub use normalized::{
    HApprovalGate, HCandidateOutcome, HEpoch, HEvalPair, HEvaluator, run_proposal_h,
};
use std::collections::BTreeMap;
use tinyhivemind_lang::{
    lineage::{self, Direction, ObjectiveScore, Verdict, VersionMetadata},
    patch::{self, Operation},
};
pub use types::*;

/// Evaluate a typed proposal through screening, selection and confirmation.
///
/// Every stopped proposal receives an archive event. Valid evaluated designs
/// additionally retain the language's exact record and pinned package. Accepted
/// designs require fresh confirmation and explicit human approval. The returned
/// activation plan is for the host to execute at a safe boundary.
///
/// # Errors
/// Returns archive failures or an invalid epoch/incumbent configuration. Host
/// evaluation failures are archived as [`ReasonCode::HostFailure`]; a
/// candidate that failed runs its incumbent completed
/// ([`Error::CandidateFailed`]) is archived as [`ReasonCode::Harmful`].
pub fn run_proposal(
    archive: &mut Archive,
    epoch: &Epoch,
    proposal: &Proposal,
    evaluator: &mut impl Evaluator,
    approval: &mut impl ApprovalGate,
) -> Result<CandidateOutcome> {
    let head = archive.head().ok_or(Error::InvalidRecord)?.clone();
    let incumbent = CandidateState {
        package: archive
            .package(&head.id)
            .ok_or(Error::InvalidRecord)?
            .clone(),
        memory_watermarks: head.memory_watermarks.clone(),
    };
    if let Some(reason) = prefilter(archive, epoch, proposal) {
        return stopped(archive, epoch, proposal, reason, None, None);
    }
    archive.declare_proposal(crate::ProposalDeclaration {
        epoch: epoch.id,
        proposal: proposal.clone(),
    })?;
    let preparation = prepare(&incumbent, proposal, evaluator);
    let (candidate, id) = match preparation {
        Ok(value) => value,
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
        parent: head.id,
        incumbent,
        candidate,
        id,
        last: None,
        runs: vec![],
        selection: vec![],
    };
    for stage in [
        EvalStage::Screen,
        EvalStage::Selection,
        EvalStage::Confirmation,
    ] {
        let (pair, comparison) = match flow.measure(stage, evaluator)? {
            Ok(measured) => measured,
            Err(reason) => return flow.stop(reason),
        };

        flow.archive.record_evaluation(crate::EvaluationEvent {
            candidate_id: flow.id.clone(),
            stage: format!("{stage:?}"),
            incumbent: pair.incumbent,
            candidate: pair.candidate.clone(),
            budget: Some(epoch.budget.clone()),
            policy: Some(EvalPolicy {
                margin: epoch.policy.margin.max(proposal.prediction.minimum_gain),
                ..epoch.policy.clone()
            }),
            comparison: Some(comparison.clone()),
        })?;
        flow.runs.clone_from(&pair.candidate);
        if stage == EvalStage::Confirmation
            && ensure_independent(&flow.selection, &pair.candidate).is_err()
        {
            flow.last = Some(comparison);
            return flow.stop(ReasonCode::NotReplicated);
        }
        let reason = decision_reason(stage, &comparison);
        flow.last = Some(comparison);
        if let Some(reason) = reason {
            return flow.stop(reason);
        }
        if stage != EvalStage::Confirmation {
            flow.selection.extend(pair.candidate.clone());
        }
        if stage == EvalStage::Confirmation {
            return flow.finish(&pair.candidate, approval);
        }
    }
    Err(Error::InvalidEvaluation)
}
fn prepare(
    incumbent: &CandidateState,
    p: &Proposal,
    evaluator: &mut impl Evaluator,
) -> std::result::Result<(CandidateState, String), ReasonCode> {
    for identity in &incumbent.package.memory.identities {
        if identity.read_only
            && p.patch.operations.iter().any(
                |op| matches!(op,Operation::Learning{identity:target,..} if *target==identity.id),
            )
        {
            return Err(ReasonCode::Guard);
        }
    }
    let applied =
        patch::apply(&incumbent.package, &p.patch).map_err(|error| patch_reason(&error))?;
    // Goal descriptions can evolve; host-observed verifier criteria cannot.
    if incumbent.package.manifest.goals.iter().any(|goal| {
        applied
            .candidate
            .manifest
            .goals
            .iter()
            .find(|other| other.id == goal.id)
            .is_none_or(|other| other.acceptance != goal.acceptance)
    }) {
        return Err(ReasonCode::Guard);
    }
    let watermarks = evaluator
        .pin_memory(incumbent, &p.patch)
        .map_err(|_| ReasonCode::HostFailure)?;
    for (identity, watermark) in &watermarks {
        let learned = p
            .patch
            .operations
            .iter()
            .any(|op| matches!(op,Operation::Learning{identity:target,..} if target==identity));
        if !learned
            && incumbent
                .memory_watermarks
                .get(identity)
                .is_some_and(|old| old != watermark)
        {
            return Err(ReasonCode::Guard);
        }
    }
    for operation in &p.patch.operations {
        if let Operation::Learning {
            identity,
            watermark,
            ..
        } = operation
            && (incumbent.memory_watermarks.get(identity) != Some(watermark)
                || watermarks
                    .get(identity)
                    .is_none_or(|next| next <= watermark))
        {
            return Err(ReasonCode::Guard);
        }
    }
    let candidate = CandidateState {
        package: applied.candidate,
        memory_watermarks: watermarks,
    };
    let id = lineage::content_id(&candidate.package, &candidate.memory_watermarks)
        .map_err(|_| ReasonCode::Unformalizable)?;
    Ok((candidate, id))
}
struct Flow<'a> {
    archive: &'a mut Archive,
    epoch: &'a Epoch,
    proposal: &'a Proposal,
    parent: String,
    incumbent: CandidateState,
    candidate: CandidateState,
    id: String,
    last: Option<Comparison>,
    selection: Vec<EvalRun>,
    runs: Vec<EvalRun>,
}
impl Flow<'_> {
    /// Runs one stage, or names why the attempt stops instead.
    fn measure(
        &mut self,
        stage: EvalStage,
        evaluator: &mut impl Evaluator,
    ) -> Result<std::result::Result<(EvalPair, Comparison), ReasonCode>> {
        let pair =
            match evaluator.evaluate(&self.incumbent, &self.candidate, stage, &self.epoch.budget) {
                Ok(pair) => pair,
                Err(error) => {
                    return failure_reason(self.archive, &error, &self.id, self.epoch.id, stage)
                        .map(Err);
                }
            };
        let mut policy = self.epoch.policy.clone();
        policy.margin = policy.margin.max(self.proposal.prediction.minimum_gain);
        let Ok(comparison) = compare(
            &pair.incumbent,
            &pair.candidate,
            &self.epoch.budget,
            &policy,
        ) else {
            return Ok(Err(ReasonCode::HostFailure));
        };
        Ok(Ok((pair, comparison)))
    }
    fn stop(self, reason: ReasonCode) -> Result<CandidateOutcome> {
        if !self.runs.is_empty() && !self.archive.records().iter().any(|r| r.id == self.id) {
            self.archive.conclude(
                self.candidate.package.clone(),
                metadata(
                    &self.parent,
                    &self.candidate,
                    self.proposal,
                    Verdict::Rejected,
                    &self.runs,
                )?,
                event(self.epoch, self.proposal, reason),
            )?;
            return Ok(CandidateOutcome {
                reason,
                candidate_id: Some(self.id),
                comparison: self.last,
                apply_at: None,
            });
        }
        stopped(
            self.archive,
            self.epoch,
            self.proposal,
            reason,
            Some(self.id),
            self.last,
        )
    }
    fn finish(
        self,
        runs: &[EvalRun],
        approval: &mut impl ApprovalGate,
    ) -> Result<CandidateOutcome> {
        let comparison = self.last.as_ref().ok_or(Error::InvalidEvaluation)?;
        let Ok(approved) = approval.approve(&self.id, self.proposal, comparison) else {
            return self.stop(ReasonCode::HostFailure);
        };
        let reason = if approved {
            ReasonCode::Accepted
        } else {
            ReasonCode::ApprovalDenied
        };
        let verdict = if approved {
            Verdict::Accepted
        } else {
            Verdict::Rejected
        };
        self.archive.conclude(
            self.candidate.package.clone(),
            metadata(&self.parent, &self.candidate, self.proposal, verdict, runs)?,
            event(self.epoch, self.proposal, reason),
        )?;
        Ok(CandidateOutcome {
            reason,
            candidate_id: Some(self.id),
            comparison: self.last,
            apply_at: approved.then(|| apply_at(self.proposal.patch.class)),
        })
    }
}
/// Names the stop for an evaluator error, keeping evidence of a candidate
/// failure.
///
/// Only [`Error::CandidateFailed`] with consistent counts is the candidate's
/// fault: it is archived as a [`crate::CandidateFailureEvent`] and stops as
/// [`ReasonCode::Harmful`]. Every other error, including malformed failure
/// counts, stays a [`ReasonCode::HostFailure`].
fn failure_reason(
    archive: &mut Archive,
    error: &Error,
    candidate_id: &str,
    epoch: u64,
    stage: EvalStage,
) -> Result<ReasonCode> {
    let Error::CandidateFailed(failure) = error else {
        return Ok(ReasonCode::HostFailure);
    };
    if failure.failed_runs == 0 || failure.failed_runs > failure.runs {
        return Ok(ReasonCode::HostFailure);
    }
    archive.record_candidate_failure(crate::CandidateFailureEvent {
        candidate_id: candidate_id.to_owned(),
        epoch,
        stage: format!("{stage:?}"),
        kind: failure.kind,
        failed_runs: failure.failed_runs,
        runs: failure.runs,
    })?;
    Ok(ReasonCode::Harmful)
}
fn decision_reason(stage: EvalStage, c: &Comparison) -> Option<ReasonCode> {
    match &c.decision {
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
    }
}
fn prefilter(archive: &Archive, epoch: &Epoch, p: &Proposal) -> Option<ReasonCode> {
    prefilter_for(archive, epoch, p, "verified_quality", Some(1.0))
}
fn prefilter_for(
    archive: &Archive,
    epoch: &Epoch,
    p: &Proposal,
    metric: &str,
    maximum_gain: Option<f64>,
) -> Option<ReasonCode> {
    if p.patch.class != epoch.class {
        return Some(ReasonCode::WrongClass);
    }
    if p.patch.operations.is_empty()
        || [&p.title, &p.mechanism, &p.risk, &p.effort]
            .iter()
            .any(|s| s.trim().is_empty())
        || p.prediction.metric != metric
        || !p.prediction.minimum_gain.is_finite()
        || p.prediction.minimum_gain <= 0.
        || maximum_gain.is_some_and(|maximum| p.prediction.minimum_gain > maximum)
        || p.prediction.nodes.is_empty()
    {
        return Some(ReasonCode::Untestable);
    }
    if p.transcript_refs.is_empty()
        || p.transcript_refs
            .iter()
            .any(|r| !epoch.transcript_refs.contains(r))
        || p.prediction
            .nodes
            .iter()
            .any(|n| !epoch.failure_nodes.contains(n))
    {
        return Some(ReasonCode::Ungrounded);
    }
    if archive.records().iter().any(|r| r.patch == p.patch)
        || archive.proposals().iter().any(|r| r.patch == p.patch)
    {
        return Some(ReasonCode::Duplicate);
    }
    if archive
        .penalties()
        .cooled(&crate::PatchShape::of(&p.patch), epoch.id, &epoch.penalty)
    {
        return Some(ReasonCode::Cooldown);
    }
    None
}
fn patch_reason(error: &patch::Error) -> ReasonCode {
    match error {
        patch::Error::ClassBoundary(_)
        | patch::Error::ForbiddenClass
        | patch::Error::Immutable
        | patch::Error::ReadOnly(_)
        | patch::Error::Contamination => ReasonCode::Guard,
        _ => ReasonCode::Unformalizable,
    }
}
fn event(epoch: &Epoch, p: &Proposal, reason: ReasonCode) -> ProposalEvent {
    ProposalEvent {
        epoch: epoch.id,
        patch: p.patch.clone(),
        rationale: format!("{}: {}", p.title, p.mechanism),
        reason: format!("{reason:?}"),
        transcript_refs: p.transcript_refs.clone(),
    }
}
fn stopped(
    archive: &mut Archive,
    epoch: &Epoch,
    p: &Proposal,
    reason: ReasonCode,
    id: Option<String>,
    comparison: Option<Comparison>,
) -> Result<CandidateOutcome> {
    archive.record_proposal(event(epoch, p, reason))?;
    Ok(CandidateOutcome {
        reason,
        candidate_id: id,
        comparison,
        apply_at: None,
    })
}
fn metadata(
    parent: &str,
    candidate: &CandidateState,
    p: &Proposal,
    verdict: Verdict,
    runs: &[EvalRun],
) -> Result<VersionMetadata> {
    // Exact per-task data lives in EvaluationEvent. This mean is only a compact archive objective.
    let count = u32::try_from(runs.len()).map_err(|_| Error::InvalidEvaluation)?;
    let quality = runs.iter().map(|r| r.quality).sum::<f64>() / f64::from(count);
    Ok(VersionMetadata {
        parent: Some(parent.into()),
        patch: p.patch.clone(),
        rationale: format!("{}: {}", p.title, p.mechanism),
        transcript_refs: p.transcript_refs.clone(),
        scores: BTreeMap::from([(
            "quality".into(),
            ObjectiveScore {
                value: quality,
                direction: Direction::Maximize,
            },
        )]),
        verdict,
        memory_watermarks: candidate.memory_watermarks.clone(),
    })
}
/// Translate edit authority into the host activation boundary.
#[must_use]
pub fn apply_at(class: PatchClass) -> ApplyAt {
    match class {
        PatchClass::Context | PatchClass::Memory => ApplyAt::NextTurn,
        PatchClass::Prompt => ApplyAt::ReplaceAgent,
        PatchClass::Roster => ApplyAt::ManageRoster,
        PatchClass::Topology | PatchClass::Policy | PatchClass::Goal | PatchClass::Integration => {
            ApplyAt::BetweenSessions
        }
    }
}
#[cfg(test)]
mod test;

#[cfg(test)]
mod test_funnel;
