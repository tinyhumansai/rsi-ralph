//! Host-owned strong verification with identity-bound artifact evidence.
//!
//! `VerifierRegistry` freezes which checks apply to which node and seat.
//! `VerifiedRunner` binds an artifact to its episode, seat, node and
//! transcript references and submits coordinator completion only after the
//! required checks pass. Process, schema, golden, Lean and pinned Lake
//! checkers run under host control with private inputs. See this directory's
//! `README.md` for the process sandbox boundary and `lake/README.md` for the
//! pinned toolchain rules.
pub(crate) mod lake;
pub use lake::identity as lake_identity;
mod runner;
mod types;
use crate::process::Exit;
use crate::{Error, Result};
pub use runner::*;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, time::Duration};
pub use types::*;

/// Computes the identity of a frozen verifier without exposing private inputs.
///
/// Process identities include executable bytes, path, arguments and limits.
/// Golden values and schemas contribute only through the returned digest.
///
/// # Errors
/// Returns a host failure if an executable cannot be read, or invalid blueprint
/// if the checker configuration cannot be serialized.
pub fn checker_identity(verifier: &HostVerifier) -> Result<String> {
    let command = |command: &CommandSpec| -> Result<serde_json::Value> {
        let bytes = std::fs::read(&command.program).map_err(|_| Error::HostFailed)?;
        Ok(serde_json::json!({
            "program": command.program,
            "executable": format!("{:x}", Sha256::digest(bytes)),
            "args": command.args,
            "workspace": command.workspace_root,
            "timeout_ms": command.timeout_ms,
            "max_output_bytes": command.max_output_bytes
        }))
    };
    let definition = match verifier {
        HostVerifier::Process(spec) => serde_json::json!({"process": command(spec)?}),
        HostVerifier::Golden { expected } => {
            serde_json::json!({"golden_sha256": format!("{:x}", Sha256::digest(expected))})
        }
        HostVerifier::Schema { schema } => serde_json::json!({"schema": schema}),
        HostVerifier::Lean {
            command: spec,
            theorem,
        } => {
            serde_json::json!({"lean": command(spec)?, "theorem": theorem})
        }
        HostVerifier::Lake {
            command: spec,
            theorem,
            toolchain,
        } => serde_json::json!({
            "lake": command(spec)?,
            "project": lake::identity(spec)?,
            "theorem": theorem,
            "toolchain": toolchain
        }),
    };
    let bytes = serde_json::to_vec(&definition).map_err(|_| Error::InvalidBlueprint)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

/// Immutable registry of authorized node checkers.
pub struct VerifierRegistry {
    declarations: BTreeMap<String, VerifierDeclaration>,
    checker_identities: BTreeMap<String, Option<String>>,
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
        let mut identities = BTreeMap::new();
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
                HostVerifier::Lake {
                    command,
                    theorem,
                    toolchain,
                } => {
                    lake::validate(command, theorem, toolchain)?;
                }
                HostVerifier::Schema { schema } => {
                    jsonschema::validator_for(schema).map_err(|_| Error::InvalidBlueprint)?;
                }
                HostVerifier::Golden { .. } => {}
            }
            let node_id = d.node_id.clone();
            let identity = match checker_identity(&d.verifier) {
                Ok(identity) => Some(identity),
                Err(Error::HostFailed) => None,
                Err(error) => return Err(error),
            };
            if map.insert(node_id.clone(), d).is_some() {
                return Err(Error::InvalidBlueprint);
            }
            identities.insert(node_id, identity);
        }
        Ok(Self {
            declarations: map,
            checker_identities: identities,
        })
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
        let verifier = self.authorize(expected, artifact)?;
        let checker_identity = checker_identity(verifier)?;
        let frozen = self
            .checker_identities
            .get(&expected.node_id)
            .ok_or(Error::InvalidBlueprint)?;
        if frozen
            .as_ref()
            .is_none_or(|frozen| frozen != &checker_identity)
        {
            return Err(Error::InvalidBlueprint);
        }
        let (passed, exit_code) = match verifier {
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
            HostVerifier::Lake {
                command,
                theorem,
                toolchain,
            } => {
                let proof =
                    std::str::from_utf8(&artifact.content).map_err(|_| Error::InvalidBlueprint)?;
                if safe_proof(proof) {
                    let source = format!("{theorem}\n{proof}\n#print axioms rsi_goal\n");
                    lake::execute(command, &source, toolchain)?
                } else {
                    (false, None)
                }
            }
        };
        let artifact_digest = format!("{:x}", Sha256::digest(&artifact.content));
        Ok(VerificationEvidence {
            binding: expected.clone(),
            artifact_digest,
            checker_identity: Some(checker_identity),
            passed,
            quality: if passed { 1.0 } else { 0.0 },
            exit_code,
        })
    }
    /// The checker `expected` authorizes for `artifact`, without running it.
    /// # Errors
    /// Returns invalid blueprint for an undeclared node or a forged binding.
    pub(crate) fn authorize(
        &self,
        expected: &VerificationBinding,
        artifact: &ArtifactSubmission,
    ) -> Result<&HostVerifier> {
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
        Ok(&d.verifier)
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
    Ok((passed && trusted_axioms(&output, false), code))
}
fn trusted_axioms(output: &[u8], lake: bool) -> bool {
    let output = std::str::from_utf8(output).unwrap_or("");
    let mut reports = output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if lake {
                crate::verification::lake::diagnostic(line)
            } else {
                Some(line)
            }
        })
        .filter(|line| {
            line.starts_with("'rsi_goal' does not depend on any axioms")
                || line.starts_with("'rsi_goal' depends on axioms:")
        });
    reports.next().is_some_and(|line| {
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
    }) && reports.next().is_none()
}
fn process(c: &CommandSpec, content: &[u8]) -> Result<(bool, Option<i32>, Vec<u8>)> {
    process_with_files(c, content, &[])
}
fn process_with_files(
    c: &CommandSpec,
    content: &[u8],
    files: &[(&str, &[u8])],
) -> Result<(bool, Option<i32>, Vec<u8>)> {
    std::fs::create_dir_all(&c.workspace_root).map_err(|_| Error::HostFailed)?;
    let root = std::fs::canonicalize(&c.workspace_root).map_err(|_| Error::HostFailed)?;
    let dir = tempfile::tempdir_in(root).map_err(|_| Error::HostFailed)?;
    let artifact = dir.path().join("artifact.lean");
    std::fs::write(&artifact, content).map_err(|_| Error::HostFailed)?;
    for (name, bytes) in files {
        std::fs::write(dir.path().join(name), bytes).map_err(|_| Error::HostFailed)?;
    }
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
        (
            "TMPDIR".to_owned(),
            dir.path().to_string_lossy().into_owned(),
        ),
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
            Ok((success && output.len() <= c.max_output_bytes, code, output))
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
