//! A host checker behind the scoring port.
//!
//! [`VerifierScorer`] submits the subject's bytes as an artifact to a frozen
//! [`VerifierRegistry`] under one fixed [`VerificationBinding`]. A pass scores
//! the checker's quality and a failure scores zero. Checkers are deterministic
//! (the same bytes give the same verdict), so it runs the checker once per
//! assessment and reports that quality for every seed. Executed and proved
//! evidence is the strongest the loop has, so it is accept-eligible.
use crate::{ArtifactSubmission, Error, VerificationBinding, VerifierRegistry};
use tinyrsi_score::{Assessment, Result as ScoreResult, Scorer};
use tinyrsi_types::Goal;

/// Scores artifact bytes by running one registered host checker.
#[derive(Debug)]
pub struct VerifierScorer {
    registry: VerifierRegistry,
    binding: VerificationBinding,
}

impl VerifierScorer {
    /// Checks subjects against the declaration `binding` names in `registry`.
    #[must_use]
    pub fn new(registry: VerifierRegistry, binding: VerificationBinding) -> Self {
        Self { registry, binding }
    }
}

impl Scorer<[u8]> for VerifierScorer {
    /// # Errors
    ///
    /// Returns `InvalidEvaluation` for no seeds or a binding the registry does
    /// not authorize, and `HostFailed` when the checker cannot run.
    fn assess(&self, subject: &[u8], _: &Goal, seeds: &[u64]) -> ScoreResult<Assessment> {
        if seeds.is_empty() {
            return Err(tinyrsi_score::Error::InvalidEvaluation);
        }
        let evidence = self
            .registry
            .verify(
                &self.binding,
                &ArtifactSubmission {
                    binding: self.binding.clone(),
                    content: subject.to_vec(),
                },
            )
            .map_err(|error| match error {
                Error::HostFailed => tinyrsi_score::Error::HostFailed,
                _ => tinyrsi_score::Error::InvalidEvaluation,
            })?;
        let quality = if evidence.passed {
            evidence.quality.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let qualities = vec![quality; seeds.len()];
        Ok(Assessment {
            eligible: Some(qualities.clone()),
            qualities,
            metric: None,
            rationale: Some(format!(
                "checker {} on node {}",
                if evidence.passed { "passed" } else { "failed" },
                self.binding.node_id
            )),
            tokens: 0,
        })
    }
}

#[cfg(test)]
mod test;
