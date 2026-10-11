//! Validated archive event folds and pinned learning ancestry.
use super::{EvaluationEvent, Event, State};
use crate::{Error, Result};
use std::collections::BTreeSet;
use tinyhivemind_lang::lineage::{self, Record, Verdict};
use tinyhivemind_lang::patch::{self, Operation};

impl State {
    pub(super) fn push(&mut self, event: Event) -> Result<()> {
        match &event {
            Event::Candidate { record, package } => {
                lineage::verify(record, package).map_err(|_| Error::InvalidRecord)?;
                self.validate_normalized_record(record, package)?;
                if self
                    .prepared
                    .get(&record.id)
                    .is_some_and(|(prepared, watermarks)| {
                        prepared != package.as_ref() || watermarks != &record.memory_watermarks
                    })
                {
                    return Err(Error::InvalidRecord);
                }
                if self.packages.contains_key(&record.id) {
                    return Err(Error::InvalidRecord);
                }
                if self.records.is_empty() {
                    if record.parent.is_some()
                        || record.verdict != Verdict::Accepted
                        || !record.patch.operations.is_empty()
                    {
                        return Err(Error::InvalidRecord);
                    }
                } else {
                    let parent = record.parent.as_ref().ok_or(Error::InvalidRecord)?;
                    let original = self.packages.get(parent).ok_or(Error::InvalidRecord)?;
                    if record.patch.operations.iter().any(|operation| {
                        matches!(operation, Operation::Learning { identity, .. }
                            if original.memory.identities.iter().any(|memory|
                                memory.id == *identity && memory.read_only))
                    }) {
                        return Err(Error::InvalidRecord);
                    }
                    patch::guard(original, package, &record.patch)
                        .map_err(|_| Error::InvalidRecord)?;
                    let parent_record = self
                        .records
                        .iter()
                        .find(|record| &record.id == parent)
                        .ok_or(Error::InvalidRecord)?;
                    validate_watermarks(parent_record, record)?;
                    if record.verdict == Verdict::Accepted && self.active.as_ref() != Some(parent) {
                        return Err(Error::InvalidRecord);
                    }
                }
                if record.verdict == Verdict::Accepted {
                    self.active = Some(record.id.clone());
                }
                self.packages
                    .insert(record.id.clone(), package.as_ref().clone());
                self.records.push(record.as_ref().clone());
            }
            Event::Prepared {
                id,
                package,
                watermarks,
            } => {
                tinyhivemind_lang::validate(package).map_err(|_| Error::InvalidRecord)?;
                if self.packages.contains_key(id)
                    || self.prepared.contains_key(id)
                    || lineage::content_id(package, watermarks).map_err(|_| Error::InvalidRecord)?
                        != *id
                {
                    return Err(Error::InvalidRecord);
                }
                self.prepared
                    .insert(id.clone(), (package.as_ref().clone(), watermarks.clone()));
            }
            Event::Activation { id } => {
                if self.revert_target()? != id {
                    return Err(Error::InvalidRecord);
                }
                self.active = Some(id.clone());
            }
            Event::Proposal { .. } => {}
            Event::Declaration { declaration } => {
                if !declaration.proposal.prediction.minimum_gain.is_finite() {
                    return Err(Error::InvalidRecord);
                }
            }
            Event::Evaluation { evaluation } => validate_evaluation(evaluation)?,
            Event::NormalizedDeclaration { declaration } => {
                self.validate_normalized_declaration(declaration)?;
            }
            Event::NormalizedEvaluation { evaluation } => self.validate_normalized(evaluation)?,
            Event::NormalizedApproval { approval } => {
                self.validate_normalized_approval(approval)?;
            }
            Event::CandidateFailure { failure } => self.validate_candidate_failure(failure)?,
            Event::Revocation { revocation } => {
                if self.active.as_ref() != Some(&revocation.candidate_id) {
                    return Err(Error::InvalidRecord);
                }
            }
        }
        self.events.push(event);
        Ok(())
    }

    /// A failure names a known candidate, a funnel stage and consistent counts.
    fn validate_candidate_failure(&self, failure: &super::CandidateFailureEvent) -> Result<()> {
        if !(self.prepared.contains_key(&failure.candidate_id)
            || self.packages.contains_key(&failure.candidate_id))
            || !matches!(
                failure.stage.as_str(),
                "Screen" | "Selection" | "Confirmation"
            )
            || failure.failed_runs == 0
            || failure.failed_runs > failure.runs
        {
            return Err(Error::InvalidRecord);
        }
        Ok(())
    }

    pub(super) fn revert_target(&self) -> Result<&str> {
        let active = self.active.as_ref().ok_or(Error::NoParent)?;
        let head = self
            .records
            .iter()
            .find(|record| &record.id == active)
            .ok_or(Error::NoParent)?;
        let parent = head.parent.as_deref().ok_or(Error::NoParent)?;
        self.records
            .iter()
            .find(|record| record.id == parent && record.verdict == Verdict::Accepted)
            .map(|record| record.id.as_str())
            .ok_or(Error::NoParent)
    }
}

pub(super) fn validate_watermarks(parent: &Record, record: &Record) -> Result<()> {
    let mut learned = BTreeSet::new();
    for operation in &record.patch.operations {
        if let Operation::Learning {
            identity,
            watermark,
            ..
        } = operation
        {
            if parent.memory_watermarks.get(identity) != Some(watermark)
                || record
                    .memory_watermarks
                    .get(identity)
                    .is_none_or(|next| next <= watermark)
            {
                return Err(Error::InvalidRecord);
            }
            learned.insert(identity);
        }
    }
    for (identity, watermark) in &record.memory_watermarks {
        if !learned.contains(identity)
            && parent
                .memory_watermarks
                .get(identity)
                .is_some_and(|previous| previous != watermark)
        {
            return Err(Error::InvalidRecord);
        }
    }
    Ok(())
}

fn validate_evaluation(event: &EvaluationEvent) -> Result<()> {
    if !event.candidate_id.starts_with("sha256:")
        || event.candidate_id.len() != 71
        || !event.candidate_id[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || event.stage.trim().is_empty()
        || event.incumbent.is_empty()
    {
        return Err(Error::InvalidRecord);
    }
    let collect = |runs: &[crate::EvalRun]| -> Result<BTreeSet<(String, String, u64)>> {
        let mut keys = BTreeSet::new();
        for run in runs {
            if run.task.trim().is_empty()
                || run.family.trim().is_empty()
                || !run.quality.is_finite()
                || !(0.0..=1.0).contains(&run.quality)
                || !keys.insert((run.task.clone(), run.family.clone(), run.seed))
            {
                return Err(Error::InvalidRecord);
            }
        }
        Ok(keys)
    };
    if collect(&event.incumbent)? != collect(&event.candidate)? {
        return Err(Error::InvalidRecord);
    }
    match (&event.budget, &event.policy, &event.comparison) {
        (None, None, None) => {}
        (Some(budget), Some(policy), Some(comparison)) => {
            let computed = crate::compare(&event.incumbent, &event.candidate, budget, policy)
                .map_err(|_| Error::InvalidRecord)?;
            if computed != *comparison {
                return Err(Error::InvalidRecord);
            }
        }
        _ => return Err(Error::InvalidRecord),
    }
    Ok(())
}
