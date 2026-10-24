//! Replay validation for exact normalized evidence and explicit human approval.

use super::{
    Archive, Event, NormalizedApprovalEvent, NormalizedDeclarationEvent, NormalizedEvaluationEvent,
    State,
};
use crate::{Error, EvalDecision, Result};
use tinyhivemind_lang::lineage::{Direction, Record, Verdict};

impl Archive {
    /// Freeze normalized policy, budget, suites, and parent before candidate output.
    ///
    /// # Errors
    /// Returns `InvalidRecord` for contradictions, duplicates, invalid policy,
    /// or attempted preregistration after a matching candidate was observed.
    /// Returns `ArchiveIo` if the frozen declaration cannot be persisted.
    pub fn declare_normalized_proposal(
        &mut self,
        declaration: NormalizedDeclarationEvent,
    ) -> Result<()> {
        self.transact(Event::NormalizedDeclaration {
            declaration: Box::new(declaration),
        })
    }

    /// Durable normalized preregistrations in insertion order.
    #[must_use]
    pub fn normalized_declarations(&self) -> Vec<&NormalizedDeclarationEvent> {
        self.state
            .events
            .iter()
            .filter_map(|event| match event {
                Event::NormalizedDeclaration { declaration } => Some(declaration.as_ref()),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn normalized_existing_candidate(
        &self,
        parent: &str,
        proposal: &crate::Proposal,
    ) -> Option<String> {
        self.state.existing_normalized_candidate(parent, proposal)
    }

    /// Persist normalized evidence bound to a prepared design and current parent.
    ///
    /// # Errors
    /// Returns `InvalidRecord` for malformed, mismatched, or altered evidence,
    /// and `ArchiveIo` for persistence failures.
    pub fn record_normalized_evaluation(
        &mut self,
        evaluation: NormalizedEvaluationEvent,
    ) -> Result<()> {
        self.transact(Event::NormalizedEvaluation {
            evaluation: Box::new(evaluation),
        })
    }

    /// Exact normalized stage evidence in insertion order.
    #[must_use]
    pub fn normalized_evaluations(&self) -> Vec<&NormalizedEvaluationEvent> {
        self.state.normalized_for(None)
    }

    /// Persist an explicit decision after independent accepted confirmation.
    ///
    /// # Errors
    /// Returns `InvalidRecord` if confirmation is absent, insufficient, reused,
    /// or different from the reviewed evidence, and `ArchiveIo` on write failure.
    pub fn record_normalized_approval(&mut self, approval: NormalizedApprovalEvent) -> Result<()> {
        self.transact(Event::NormalizedApproval {
            approval: Box::new(approval),
        })
    }

    /// Human decisions bound to normalized confirmation evidence.
    #[must_use]
    pub fn normalized_approvals(&self) -> Vec<&NormalizedApprovalEvent> {
        self.state
            .events
            .iter()
            .filter_map(|event| match event {
                Event::NormalizedApproval { approval } => Some(approval.as_ref()),
                _ => None,
            })
            .collect()
    }
}

impl State {
    fn existing_normalized_candidate(
        &self,
        parent: &str,
        proposal: &crate::Proposal,
    ) -> Option<String> {
        let original = self.packages.get(parent)?;
        self.packages
            .iter()
            .chain(self.prepared.iter().map(|(id, (pkg, _))| (id, pkg)))
            .find_map(|(id, pkg)| {
                tinyhivemind_lang::patch::guard(original, pkg, &proposal.patch)
                    .is_ok()
                    .then(|| id.clone())
            })
    }

    pub(super) fn validate_normalized_declaration(
        &self,
        d: &NormalizedDeclarationEvent,
    ) -> Result<()> {
        d.policy.build().map_err(|_| Error::InvalidRecord)?;
        let suites: std::collections::BTreeSet<_> = d.suite_ids.values().collect();
        if self.active.as_ref() != Some(&d.incumbent_id)
            || d.budget.max_tokens == 0
            || d.suite_ids.len() != 3
            || suites.len() != 3
            || ["Screen", "Selection", "Confirmation"]
                .iter()
                .any(|stage| d.suite_ids.get(*stage).is_none_or(|s| s.trim().is_empty()))
            || d.proposal.prediction.metric != "normalized_h"
            || !d.proposal.prediction.minimum_gain.is_finite()
            || d.proposal.prediction.minimum_gain <= 0.
            || self
                .existing_normalized_candidate(&d.incumbent_id, &d.proposal)
                .is_some()
        {
            return invalid();
        }
        let mut proposal_declared = false;
        for event in &self.events {
            match event {
                Event::Declaration { declaration }
                    if declaration.epoch == d.epoch && declaration.proposal == d.proposal =>
                {
                    proposal_declared = true;
                }
                Event::NormalizedDeclaration {
                    declaration: previous,
                } if previous.epoch == d.epoch => {
                    if previous.incumbent_id != d.incumbent_id
                        || previous.policy != d.policy
                        || previous.budget != d.budget
                        || previous.suite_ids != d.suite_ids
                        || previous.proposal == d.proposal
                    {
                        return invalid();
                    }
                }
                Event::Proposal { proposal }
                    if proposal.epoch == d.epoch && proposal.patch == d.proposal.patch =>
                {
                    return invalid();
                }
                _ => {}
            }
        }
        if !proposal_declared {
            return invalid();
        }
        Ok(())
    }

    fn normalized_for(&self, id: Option<&str>) -> Vec<&NormalizedEvaluationEvent> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::NormalizedEvaluation { evaluation }
                    if id.is_none_or(|id| evaluation.candidate_id == id) =>
                {
                    Some(evaluation.as_ref())
                }
                _ => None,
            })
            .collect()
    }

    pub(super) fn validate_normalized(&self, e: &NormalizedEvaluationEvent) -> Result<()> {
        let policy = e.policy.build().map_err(|_| Error::InvalidRecord)?;
        let stage_index = match e.stage.as_str() {
            "Screen" => 0,
            "Selection" => 1,
            "Confirmation" => 2,
            _ => return invalid(),
        };
        if self.active.as_ref() != Some(&e.incumbent_id)
            || !self.prepared.contains_key(&e.candidate_id)
            || self.packages.contains_key(&e.candidate_id)
            || e.suite_ids.len() != 3
            || ["Screen", "Selection", "Confirmation"].iter().any(|stage| {
                e.suite_ids
                    .get(*stage)
                    .is_none_or(|id| id.trim().is_empty())
            })
            || e.suite_ids.get(&e.stage) != Some(&e.suite_id)
            || e.proposal.prediction.metric != "normalized_h"
            || !e.proposal.prediction.minimum_gain.is_finite()
            || e.proposal.prediction.minimum_gain <= 0.0
            || e.policy.margin < e.proposal.prediction.minimum_gain
        {
            return invalid();
        }
        let declaration = self
            .events
            .iter()
            .find_map(|event| match event {
                Event::NormalizedDeclaration { declaration }
                    if declaration.epoch == e.epoch && declaration.proposal == e.proposal =>
                {
                    Some(declaration.as_ref())
                }
                _ => None,
            })
            .ok_or(Error::InvalidRecord)?;
        let mut frozen_policy = declaration.policy.clone();
        frozen_policy.margin = frozen_policy.margin.max(e.proposal.prediction.minimum_gain);
        if declaration.incumbent_id != e.incumbent_id
            || declaration.budget != e.budget
            || declaration.suite_ids != e.suite_ids
            || frozen_policy != e.policy
        {
            return invalid();
        }
        let original = self
            .packages
            .get(&e.incumbent_id)
            .ok_or(Error::InvalidRecord)?;
        let (candidate, _) = self
            .prepared
            .get(&e.candidate_id)
            .ok_or(Error::InvalidRecord)?;
        tinyhivemind_lang::patch::guard(original, candidate, &e.proposal.patch)
            .map_err(|_| Error::InvalidRecord)?;
        let previous = self.normalized_for(Some(&e.candidate_id));
        if previous.len() != stage_index {
            return invalid();
        }
        if let Some(first) = previous.first()
            && (first.incumbent_id != e.incumbent_id
                || first.epoch != e.epoch
                || first.proposal != e.proposal
                || first.policy != e.policy
                || first.budget != e.budget
                || first.suite_ids != e.suite_ids)
        {
            return invalid();
        }
        if previous.last().is_some_and(|last| {
            matches!(last.comparison.decision, EvalDecision::Reject { .. })
                || (last.stage == "Selection"
                    && !matches!(last.comparison.decision, EvalDecision::Accept { .. }))
        }) {
            return invalid();
        }
        let computed = crate::compare_h(
            &e.incumbent,
            &e.candidate,
            &e.incumbent_health,
            &e.candidate_health,
            &e.budget,
            &policy,
        )
        .map_err(|_| Error::InvalidRecord)?;
        if computed != e.comparison {
            return invalid();
        }
        Ok(())
    }

    pub(super) fn validate_normalized_approval(
        &self,
        approval: &NormalizedApprovalEvent,
    ) -> Result<()> {
        let stages = self.normalized_for(Some(&approval.candidate_id));
        let confirmation = stages.last().ok_or(Error::InvalidRecord)?;
        if stages.len() != 3
            || confirmation.stage != "Confirmation"
            || confirmation.comparison != approval.comparison
            || !matches!(
                confirmation.comparison.decision,
                EvalDecision::Accept { .. }
            )
            || self.active.as_ref() != Some(&confirmation.incumbent_id)
            || self.events.iter().any(|event| {
                matches!(event, Event::NormalizedApproval { approval: previous }
                if previous.candidate_id == approval.candidate_id)
            })
        {
            return invalid();
        }
        let selection: Vec<_> = stages
            .iter()
            .take(2)
            .flat_map(|event| event.candidate.clone())
            .collect();
        crate::ensure_independent(&selection, &confirmation.candidate)
            .map_err(|_| Error::InvalidRecord)
    }

    pub(super) fn validate_normalized_record(
        &self,
        record: &Record,
        package: &tinyhivemind_lang::Package,
    ) -> Result<()> {
        let stages = self.normalized_for(Some(&record.id));
        let Some(last) = stages.last() else {
            let normalized = self.events.iter().any(|event| match event {
                Event::NormalizedDeclaration { declaration }
                    if record.parent.as_ref() == Some(&declaration.incumbent_id) =>
                {
                    self.packages
                        .get(&declaration.incumbent_id)
                        .is_some_and(|original| {
                            tinyhivemind_lang::patch::guard(
                                original,
                                package,
                                &declaration.proposal.patch,
                            )
                            .is_ok()
                        })
                }
                _ => false,
            });
            return if normalized { invalid() } else { Ok(()) };
        };
        let score = record
            .scores
            .get("normalized_h")
            .ok_or(Error::InvalidRecord)?;
        if record.parent.as_ref() != Some(&last.incumbent_id)
            || record.patch != last.proposal.patch
            || score.direction != Direction::Maximize
            || score.value.to_bits() != last.comparison.candidate.value.to_bits()
        {
            return invalid();
        }
        if record.verdict == Verdict::Accepted && !self.events.iter().any(|event| matches!(event,
            Event::NormalizedApproval { approval } if approval.candidate_id == record.id && approval.approved)) {
            return invalid();
        }
        Ok(())
    }
}

fn invalid<T>() -> Result<T> {
    Err(Error::InvalidRecord)
}
