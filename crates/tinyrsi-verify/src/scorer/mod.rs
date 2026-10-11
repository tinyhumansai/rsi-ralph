//! A host checker behind the scoring port.
//!
//! [`VerifierScorer`] submits the subject's bytes as an artifact to a frozen
//! [`VerifierRegistry`] under one fixed [`VerificationBinding`]. A pass scores
//! the checker's quality and a failure scores zero. A proof for a Lean or Lake
//! checker that is not valid UTF-8 is a failure too, like a proof the checker
//! rejects; only an unauthorized binding, a broken declaration, or a checker
//! that cannot run is an error. Checkers are deterministic
//! (the same bytes give the same verdict), so it runs the checker once per
//! assessment and reports that quality for every seed. Executed and proved
//! evidence is the strongest the loop has, so it is accept-eligible.
use crate::{ArtifactSubmission, Error, HostVerifier, VerificationBinding, VerifierRegistry};
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
    /// not authorize, and `HostFailed` when the checker cannot run. A subject
    /// the checker rejects, including a proof that is not valid UTF-8, is not
    /// an error: it scores zero.
    fn assess(&self, subject: &[u8], _: &Goal, seeds: &[u64]) -> ScoreResult<Assessment> {
        if seeds.is_empty() {
            return Err(tinyrsi_score::Error::InvalidEvaluation);
        }
        let artifact = ArtifactSubmission {
            binding: self.binding.clone(),
            content: subject.to_vec(),
        };
        let verifier = self
            .registry
            .authorize(&self.binding, &artifact)
            .map_err(|error| score_error(&error))?;
        // Proof checkers read the artifact as text, so bytes that are not
        // UTF-8 are a rejected proof rather than a broken configuration.
        let undecodable_proof = matches!(
            verifier,
            HostVerifier::Lean { .. } | HostVerifier::Lake { .. }
        ) && std::str::from_utf8(subject).is_err();
        let (passed, quality) = if undecodable_proof {
            (false, 0.0)
        } else {
            let evidence = self
                .registry
                .verify(&self.binding, &artifact)
                .map_err(|error| score_error(&error))?;
            let quality = if evidence.passed {
                evidence.quality.clamp(0.0, 1.0)
            } else {
                0.0
            };
            (evidence.passed, quality)
        };
        let qualities = vec![quality; seeds.len()];
        Ok(Assessment {
            eligible: Some(qualities.clone()),
            qualities,
            metric: None,
            rationale: Some(format!(
                "checker {} on node {}",
                if passed { "passed" } else { "failed" },
                self.binding.node_id
            )),
            tokens: 0,
        })
    }
}

/// Maps a verification error onto the scoring port's error.
fn score_error(error: &Error) -> tinyrsi_score::Error {
    match error {
        Error::HostFailed => tinyrsi_score::Error::HostFailed,
        _ => tinyrsi_score::Error::InvalidEvaluation,
    }
}

#[cfg(test)]
mod test;
