//! Observe exact-answer checker labels without exporting private sample artifacts.
use super::types::Bank;
use rsi_ralph_host::{
    ArtifactSubmission, Error, HostVerifier, Result, VerificationBinding, VerifierDeclaration,
    VerifierRegistry,
};
use std::collections::{BTreeMap, BTreeSet};

/// Require two actual checker observations per task: correct and off-by-one.
///
/// These automatic labels support future judge studies. They do not establish
/// that a judge or a model's probability predictions are calibrated.
pub(super) fn verify(bytes: &[u8]) -> Result<usize> {
    let bank: Bank = serde_json::from_slice(bytes).map_err(|_| Error::InvalidEvaluation)?;
    let rows: BTreeMap<_, _> = bank
        .stages
        .values()
        .flatten()
        .map(|r| (r.id.as_str(), r))
        .collect();
    let mut seen = BTreeSet::new();
    for sample in &bank.calibration {
        let row = rows
            .get(sample.task_id.as_str())
            .ok_or(Error::InvalidEvaluation)?;
        let number: i128 = row
            .expected
            .trim_end()
            .parse()
            .map_err(|_| Error::InvalidEvaluation)?;
        let wrong = number.checked_add(1).ok_or(Error::InvalidEvaluation)?;
        let expected_artifact = if sample.passed {
            row.expected.clone()
        } else {
            format!("{wrong}\n")
        };
        if sample.artifact != expected_artifact || !seen.insert((&sample.task_id, sample.passed)) {
            return Err(Error::InvalidEvaluation);
        }
        let registry = VerifierRegistry::new(vec![VerifierDeclaration {
            node_id: "artifact".into(),
            owner_seat: row.seat.clone(),
            verifier: HostVerifier::Golden {
                expected: row.expected.as_bytes().to_vec(),
            },
        }])?;
        let binding = VerificationBinding {
            episode_id: format!("calibration:{}", row.id),
            seat_id: row.seat.clone(),
            node_id: "artifact".into(),
            artifact_id: format!("sample:{}", sample.passed),
            transcript_refs: vec![1],
        };
        let evidence = registry.verify(
            &binding,
            &ArtifactSubmission {
                binding: binding.clone(),
                content: sample.artifact.as_bytes().to_vec(),
            },
        )?;
        if evidence.passed != sample.passed {
            return Err(Error::InvalidEvaluation);
        }
    }
    if seen.len() != rows.len() * 2 {
        return Err(Error::InvalidEvaluation);
    }
    Ok(seen.len())
}
