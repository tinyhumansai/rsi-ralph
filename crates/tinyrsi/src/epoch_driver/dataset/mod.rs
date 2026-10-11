//! Replay-derived selection datasets; generated prose and held-out measurements stay private.
mod metrics;
mod projection;
mod types;
use super::{EpochJournal, types::Entry};
use crate::{Archive, Error, ModelIdentity, Result};
pub(super) use types::Origin;
pub use types::*;

impl EpochJournal {
    /// Freezes optional probabilities before any host evaluation or preparation.
    /// Identical reads/re-registration are idempotent, including after evaluation.
    /// # Errors
    /// Rejects invalid probabilities, length mismatch, changed priors, an unrelated
    /// archive, or first registration after evaluation/declaration has begun.
    pub fn register_proposal_priors(
        &mut self,
        epoch: u64,
        priors: &[Option<f64>],
        archive: &Archive,
    ) -> Result<()> {
        let mut entry = self
            .state
            .entries
            .get(&epoch)
            .cloned()
            .ok_or(Error::InvalidProposal)?;
        let prepared = entry.prepared.as_ref().ok_or(Error::InvalidProposal)?;
        if archive.package(&entry.incumbent_id).is_none()
            || priors.len() != prepared.proposals.len()
            || priors.iter().flatten().any(|p| !probability(*p))
        {
            return Err(Error::InvalidProposal);
        }
        if let Some(previous) = &entry.priors {
            return if previous == priors {
                Ok(())
            } else {
                Err(Error::InvalidProposal)
            };
        }
        if entry.evaluating
            || !entry.outcome.candidates.is_empty()
            || archive
                .normalized_declarations()
                .iter()
                .any(|d| d.epoch == epoch && d.incumbent_id == entry.incumbent_id)
            || archive
                .normalized_evaluations()
                .iter()
                .any(|e| e.epoch == epoch && e.incumbent_id == entry.incumbent_id)
        {
            return Err(Error::InvalidProposal);
        }
        entry.priors = Some(priors.to_vec());
        self.commit_entry(entry)
    }
    /// Projects prepared occurrences and exact replay-validated host evidence.
    /// Returns `None` for an unknown epoch or legacy intent without a known arm.
    /// Failed or interrupted generation retains provenance with an empty typed row list.
    /// # Errors
    /// Rejects an unrelated archive, contradictory identities, corrupt receipts,
    /// or evidence inconsistent with the epoch's frozen selection configuration.
    pub fn proposal_dataset(
        &self,
        epoch: u64,
        archive: &Archive,
    ) -> Result<Option<ProposalOutcomeDataset>> {
        let Some(entry) = self.state.entries.get(&epoch) else {
            return Ok(None);
        };
        if entry.prepared.is_none() && entry.generation_mode.is_none() {
            return Ok(None);
        }
        projection::project(entry, self.generation_receipts(epoch)?, archive).map(Some)
    }
}
/// Accepts finite probability forecasts without interpreting gain claims as priors.
pub(super) fn probability(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
/// Limits declared public provider identifiers to bounded configuration labels.
pub(super) fn model_valid(model: &ModelIdentity) -> bool {
    [&model.provider, &model.model].iter().all(|label| {
        !label.is_empty()
            && label.len() <= 128
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/:._-".contains(&b))
    })
}
/// Checks private provenance, occurrence uniqueness and frozen prior shape on replay.
pub(super) fn validate(entry: &Entry) -> Result<()> {
    let prepared = entry.prepared.as_ref();
    if entry
        .model
        .as_ref()
        .is_some_and(|model| !model_valid(model))
        || (entry.generation_seed.is_some() && entry.selection_binding.is_none())
        || entry.priors.as_ref().is_some_and(|priors| {
            prepared.is_none_or(|p| priors.len() != p.proposals.len())
                || priors.iter().flatten().any(|p| !probability(*p))
        })
        || entry.proposal_sources.as_ref().is_some_and(|sources| {
            prepared.is_none_or(|p| sources.len() != p.proposals.len())
                || entry.generation_seed.is_none()
                || sources.iter().any(|s| !digest(&s.run_id))
                || sources
                    .iter()
                    .map(|s| &s.occurrence)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != sources.len()
        })
    {
        return Err(Error::ArchiveCorrupt);
    }
    if let Some(binding) = &entry.selection_binding {
        binding.policy.build().map_err(|_| Error::ArchiveCorrupt)?;
        if binding.budget.max_tokens == 0
            || binding.suites.len() != 3
            || ["Screen", "Selection", "Confirmation"]
                .iter()
                .any(|s| binding.suites.get(*s).is_none_or(String::is_empty))
        {
            return Err(Error::ArchiveCorrupt);
        }
    }
    Ok(())
}
/// Recognizes the canonical lowercase SHA-256 form used for public identities.
pub(super) fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SelectionBinding {
    pub policy: crate::ScoringPolicySpec,
    pub budget: crate::Budget,
    pub suites: std::collections::BTreeMap<String, String>,
}
impl From<&crate::HEpoch> for SelectionBinding {
    fn from(epoch: &crate::HEpoch) -> Self {
        Self {
            policy: epoch.policy.clone(),
            budget: epoch.epoch.budget.clone(),
            suites: epoch.suite_ids.clone(),
        }
    }
}
#[cfg(test)]
mod test;
