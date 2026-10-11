//! Host-owned strong verification with identity-bound artifact evidence.
mod runner;
mod types;
use crate::{Error, Result};
pub use runner::*;
use sha2::{Digest, Sha256};
use crate::process::Exit;
use std::{collections::BTreeMap, time::Duration};
pub use types::*;
/// Immutable registry of authorized node checkers.
pub struct VerifierRegistry {
    declarations: BTreeMap<String, VerifierDeclaration>,
}
impl VerifierRegistry {
    /// Freezes checker declarations and rejects ambiguous or unsafe settings.
    /// # Errors
    /// Returns invalid blueprint for empty/duplicate identities or invalid checks.
    pub fn new(declarations: Vec<VerifierDeclaration>) -> Result<Self> {
        if declarations.is_empty() {
            return Err(Error::InvalidBlueprint);
        }
        let mut map = BTreeMap::new();
        for d in declarations {
            if d.node_id.trim().is_empty() || d.owner_seat.trim().is_empty() {
                return Err(Error::InvalidBlueprint);
            }
            match &d.verifier {
                HostVerifier::Process(c) => validate_command(c)?,
                HostVerifier::Lean { command, theorem } => {
                    validate_command(command)?;
                    if theorem.trim().is_empty()
                        || !theorem.contains("theorem rsi_goal")
                        || !theorem.trim_end().ends_with(":=")
                    {
                        return Err(Error::InvalidBlueprint);
                    }
                }
                HostVerifier::Schema { schema } => {
                    jsonschema::validator_for(schema).map_err(|_| Error::InvalidBlueprint)?;
                }
                HostVerifier::Golden { .. } => {}
            }
            if map.insert(d.node_id.clone(), d).is_some() {
                return Err(Error::InvalidBlueprint);
            }
        }
        Ok(Self { declarations: map })
    }
    /// Frozen declarations, in deterministic node order.
    pub fn declarations(&self) -> impl Iterator<Item = &VerifierDeclaration> {
        self.declarations.values()
    }
    /// Executes the checker only after matching the host authorization exactly.
    /// # Errors
    /// Returns invalid blueprint for forged bindings and host failure for I/O errors.
    pub fn verify(
        &self,
        expected: &VerificationBinding,
        artifact: &ArtifactSubmission,
    ) -> Result<VerificationEvidence> {
        let d = self
            .declarations
            .get(&expected.node_id)
            .ok_or(Error::InvalidBlueprint)?;
        if artifact.binding != *expected
            || expected.seat_id != d.owner_seat
            || expected.episode_id.is_empty()
            || expected.artifact_id.is_empty()
            || expected.transcript_refs.is_empty()
            || expected.transcript_refs.windows(2).any(|w| w[0] >= w[1])
        {
            return Err(Error::InvalidBlueprint);
        }
        let (passed, exit_code) = match &d.verifier {
            HostVerifier::Golden { expected } => (artifact.content == *expected, None),
            HostVerifier::Schema { schema } => {
                let validator =
                    jsonschema::validator_for(schema).map_err(|_| Error::InvalidBlueprint)?;
                (
                    serde_json::from_slice::<serde_json::Value>(&artifact.content)
                        .is_ok_and(|v| validator.is_valid(&v)),
                    None,
                )
            }
            HostVerifier::Process(c) => execute(c, &artifact.content)?,
            HostVerifier::Lean { command, theorem } => {
                let proof =
                    std::str::from_utf8(&artifact.content).map_err(|_| Error::InvalidBlueprint)?;
                if safe_proof(proof) {
                    let source = format!("{theorem}\n{proof}\n#print axioms rsi_goal\n");
                    execute_lean(command, source.as_bytes())?
                } else {
                    (false, None)
                }
            }
        };
        let artifact_digest = format!("{:x}", Sha256::digest(&artifact.content));
        Ok(VerificationEvidence {
            binding: expected.clone(),
            artifact_digest,
            passed,
            quality: if passed { 1.0 } else { 0.0 },
            exit_code,
        })
    }
}
fn validate_command(c: &CommandSpec) -> Result<()> {
    if c.program.trim().is_empty() || c.timeout_ms == 0 || c.max_output_bytes == 0 {
        return Err(Error::InvalidBlueprint);
    }
    Ok(())
}
fn safe_proof(proof: &str) -> bool {
    let banned = [
        "sorry",
        "admit",
        "axiom",
        "unsafe",
        "set_option",
        "initialize",
        "run_tac",
        "run_elab",
        "import",
        "theorem",
        "def",
        "opaque",
        "constant",
        "namespace",
        "end",
        "#",
        "/--",
        "/-",
        "--",
    ];
    proof.trim_start().starts_with("by") && !banned.iter().any(|word| proof.contains(word))
}
fn execute(c: &CommandSpec, content: &[u8]) -> Result<(bool, Option<i32>)> {
    let (passed, code, _) = process(c, content)?;
    Ok((passed, code))
}
fn execute_lean(c: &CommandSpec, content: &[u8]) -> Result<(bool, Option<i32>)> {
    let (passed, code, output) = process(c, content)?;
    // Only the stock kernel axiom report for this frozen theorem is accepted.
    let output = std::str::from_utf8(&output).unwrap_or("");
    let mut reports = output.lines().map(str::trim).filter(|line| {
        line.starts_with("'rsi_goal' does not depend on any axioms")
            || line.starts_with("'rsi_goal' depends on axioms:")
    });
    let trusted = reports.next().is_some_and(|line| {
        line == "'rsi_goal' does not depend on any axioms"
            || line
                .strip_prefix("'rsi_goal' depends on axioms:")
                .is_some_and(|list| {
                    let list = list.trim();
                    list.starts_with('[')
                        && list.ends_with(']')
                        && list[1..list.len() - 1].split(',').all(|axiom| {
                            matches!(
                                axiom.trim(),
                                "" | "propext" | "Classical.choice" | "Quot.sound"
                            )
                        })
                })
    }) && reports.next().is_none();
    Ok((passed && trusted, code))
}
fn process(c: &CommandSpec, content: &[u8]) -> Result<(bool, Option<i32>, Vec<u8>)> {
    std::fs::create_dir_all(&c.workspace_root).map_err(|_| Error::HostFailed)?;
    let root = std::fs::canonicalize(&c.workspace_root).map_err(|_| Error::HostFailed)?;
    let dir = tempfile::tempdir_in(root).map_err(|_| Error::HostFailed)?;
    let artifact = dir.path().join("artifact.lean");
    std::fs::write(&artifact, content).map_err(|_| Error::HostFailed)?;
    let args: Vec<_> = c
        .args
        .iter()
        .map(|arg| {
            arg.replace("{artifact}", &artifact.to_string_lossy())
                .replace("{workspace}", &dir.path().to_string_lossy())
        })
        .collect();
    let env = [
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ("TMPDIR".to_owned(), dir.path().to_string_lossy().into_owned()),
    ];
    let outcome = crate::process::run(&crate::process::ProcessSpec {
        program: c.program.as_ref(),
        args: &args,
        cwd: dir.path(),
        scratch: dir.path(),
        env: &env,
        timeout: Duration::from_millis(c.timeout_ms),
        cap: u64::try_from(c.max_output_bytes).map_err(|_| Error::InvalidBlueprint)?,
    })?;
    match outcome.exit {
        Exit::TimedOut | Exit::OutputExceeded => Ok((false, None, vec![])),
        Exit::Exited { success, code } => {
            let mut output = outcome.stdout;
            output.extend(outcome.stderr);
            Ok((
                success && output.len() <= c.max_output_bytes,
                code,
                output,
            ))
        }
    }
}
#[cfg(test)]
mod test;

impl std::fmt::Debug for VerifierRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifierRegistry")
            .field("nodes", &self.declarations.len())
            .finish_non_exhaustive()
    }
}
