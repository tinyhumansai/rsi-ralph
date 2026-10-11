//! Validation and canonical fingerprints of immutable preregistration.

use super::{ExperimentProtocol, ExperimentProtocolSpec};
use crate::{Error, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

impl ExperimentProtocol {
    /// Freeze an experiment before executing any model or verifier callback.
    ///
    /// Requires at least three independent trials, twenty total attempts, two
    /// distinct anchor seeds per trial, distinct suite/configuration identities,
    /// finite policies, and nonzero equal caps. All three ablations are mandatory.
    ///
    /// # Errors
    /// Returns [`Error::InvalidEvaluation`] for invalid or overflowing protocol
    /// data, or a scoring specification that fails its validated constructor.
    pub fn new(spec: ExperimentProtocolSpec) -> Result<Self> {
        let identifiers = [
            &spec.original_config_id,
            &spec.evolved_config_id,
            &spec.initial_hive_id,
            &spec.suites.selection,
            &spec.suites.confirmation,
            &spec.suites.anchor,
        ];
        let suites = [
            &spec.suites.selection,
            &spec.suites.confirmation,
            &spec.suites.anchor,
        ];
        if identifiers.iter().any(|id| id.trim().is_empty())
            || spec.original_config_id == spec.evolved_config_id
            || suites.into_iter().collect::<BTreeSet<_>>().len() != 3
            || spec.trials.len() < 3
            || spec.attempts_per_trial == 0
            || spec.generation_cap == 0
            || spec.evaluation_cap == 0
            || spec.per_run_budget.max_tokens == 0
            || !spec.efficiency_margin.is_finite()
            || spec.efficiency_margin < 0.
        {
            return Err(Error::InvalidEvaluation);
        }
        spec.scoring.build()?;
        let mut generation = BTreeSet::new();
        let mut evaluation = BTreeSet::new();
        for trial in &spec.trials {
            if !generation.insert(trial.proposal_seed) || trial.evaluation_seeds.len() < 2 {
                return Err(Error::InvalidEvaluation);
            }
            for seed in &trial.evaluation_seeds {
                if !evaluation.insert(*seed) {
                    return Err(Error::InvalidEvaluation);
                }
            }
        }
        let trial_count = u64::try_from(spec.trials.len()).map_err(|_| Error::InvalidEvaluation)?;
        let assignments = trial_count.checked_mul(6).ok_or(Error::InvalidEvaluation)?;
        let attempts = assignments
            .checked_mul(u64::from(spec.attempts_per_trial))
            .ok_or(Error::InvalidEvaluation)?;
        if attempts < 20 {
            return Err(Error::InvalidEvaluation);
        }
        // Prove that all conservative reservations and final report totals fit.
        assignments
            .checked_mul(spec.generation_cap)
            .ok_or(Error::InvalidEvaluation)?;
        assignments
            .checked_mul(spec.evaluation_cap)
            .ok_or(Error::InvalidEvaluation)?;
        spec.generation_cap
            .checked_add(spec.evaluation_cap)
            .ok_or(Error::InvalidEvaluation)?;
        let bytes = serde_json::to_vec(&spec).map_err(|_| Error::InvalidEvaluation)?;
        let fingerprint = format!("{:x}", Sha256::digest(bytes));
        Ok(Self { spec, fingerprint })
    }

    /// Immutable preregistered specification; changes require a new protocol.
    #[must_use]
    pub fn spec(&self) -> &ExperimentProtocolSpec {
        &self.spec
    }

    /// SHA-256 fingerprint computed before model outputs exist.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}
