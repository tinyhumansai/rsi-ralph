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
use std::{
    collections::BTreeMap,
    fs::File,
    io,
    path::{Path, PathBuf},
    time::Duration,
};
pub use types::*;

/// Computes the identity of a frozen verifier without exposing private inputs.
///
/// Process identities include available executable bytes, path, arguments and
/// limits. A missing executable has a distinct unavailable identity and cannot
/// produce a successful check.
/// Golden values and schemas contribute only through the returned digest.
///
/// # Errors
/// Returns invalid blueprint if the checker configuration cannot be
/// serialized or a pinned Lake identity cannot be read.
pub fn checker_identity(verifier: &HostVerifier) -> Result<String> {
    checker_identity_with_executable(verifier, None, None)
}

fn checker_identity_with_executable(
    verifier: &HostVerifier,
    executable_override: Option<&Path>,
    compiler_override: Option<&Path>,
) -> Result<String> {
    let command = |command: &CommandSpec| -> Result<serde_json::Value> {
        let workspace = command
            .workspace_root
            .to_str()
            .ok_or(Error::InvalidBlueprint)?;
        let executable = match executable_override {
            Some(path) => path.to_path_buf(),
            None => resolve_executable(&command.program)?,
        };
        let metadata = std::fs::metadata(&executable).map_err(|_| Error::InvalidBlueprint)?;
        if !metadata.is_file() || !is_executable(&metadata) {
            return Err(Error::InvalidBlueprint);
        }
        let mut file = File::open(executable).map_err(|_| Error::InvalidBlueprint)?;
        let mut hash = Sha256::new();
        io::copy(&mut file, &mut hash).map_err(|_| Error::InvalidBlueprint)?;
        let executable = format!("{:x}", hash.finalize());
        Ok(serde_json::json!({
            "program": command.program,
            "executable": executable,
            "args": command.args,
            "workspace": workspace,
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
            "project": lake::identity_with_compiler(spec, compiler_override)?,
            "theorem": theorem,
            "toolchain": toolchain
        }),
    };
    let bytes = serde_json::to_vec(&definition).map_err(|_| Error::InvalidBlueprint)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_: &std::fs::Metadata) -> bool {
    true
}

/// Resolves the executable using the same fixed PATH provided to child checks.
fn resolve_executable(program: &str) -> Result<PathBuf> {
    let path = Path::new(program);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    if path.components().count() > 1 {
        return Err(Error::InvalidBlueprint);
    }
    ["/usr/bin", "/bin"]
        .into_iter()
        .map(|directory| Path::new(directory).join(path))
        .find(|candidate| candidate.is_file())
        .ok_or(Error::InvalidBlueprint)
}

/// Immutable registry of authorized node checkers.
pub struct VerifierRegistry {
    declarations: BTreeMap<String, VerifierDeclaration>,
    checker_identities: BTreeMap<String, String>,
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
            let identity = checker_identity(&d.verifier)?;
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
        let staged = match verifier {
            HostVerifier::Process(command) | HostVerifier::Lean { command, .. } => {
                Some(stage_executable(command)?)
            }
            _ => None,
        };
        let staged_lake = match verifier {
            HostVerifier::Lake { command, .. } => Some(lake::stage(command)?),
            _ => None,
        };
        let executable = staged
            .as_ref()
            .map(|(path, _)| path.as_path())
            .or_else(|| staged_lake.as_ref().map(|stage| stage.lake.as_path()));
        let compiler = staged_lake.as_ref().map(|stage| stage.lean.as_path());
        let checker_identity = checker_identity_with_executable(verifier, executable, compiler)?;
        let frozen = self
            .checker_identities
            .get(&expected.node_id)
            .ok_or(Error::InvalidBlueprint)?;
        if frozen != &checker_identity {
            return Err(Error::InvalidBlueprint);
        }
        let mut execution_verifier = verifier.clone();
        if let Some((path, _)) = &staged {
            match &mut execution_verifier {
                HostVerifier::Process(command) | HostVerifier::Lean { command, .. } => {
                    command.program = path.to_string_lossy().into_owned();
                }
                _ => {}
            }
        }
        if let (Some(stage), HostVerifier::Lake { command, .. }) =
            (&staged_lake, &mut execution_verifier)
        {
            command.program = stage.lake.to_string_lossy().into_owned();
        }
        let (passed, exit_code) = match &execution_verifier {
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

/// Copies the resolved checker into a private directory so the bytes hashed
/// for evidence are the same file passed to the child process.
fn stage_executable(command: &CommandSpec) -> Result<(PathBuf, tempfile::TempDir)> {
    let source = resolve_executable(&command.program)?;
    let metadata = std::fs::metadata(&source).map_err(|_| Error::InvalidBlueprint)?;
    if !metadata.is_file() || !is_executable(&metadata) {
        return Err(Error::InvalidBlueprint);
    }
    std::fs::create_dir_all(&command.workspace_root).map_err(|_| Error::HostFailed)?;
    let directory = tempfile::tempdir_in(&command.workspace_root).map_err(|_| Error::HostFailed)?;
    let source_parent = source.parent().ok_or(Error::InvalidBlueprint)?;
    if let Some(source_root) = source_parent.parent() {
        for entry in std::fs::read_dir(source_root).map_err(|_| Error::HostFailed)? {
            let entry = entry.map_err(|_| Error::HostFailed)?;
            if entry.path() == source_parent {
                continue;
            }
            stage_link(&entry.path(), &directory.path().join(entry.file_name()))
                .map_err(|_| Error::HostFailed)?;
        }
    }
    let staged_parent = staged_parent(directory.path(), source_parent)?;
    if source_parent != Path::new("/") {
        std::fs::create_dir(&staged_parent).map_err(|_| Error::HostFailed)?;
    }
    for entry in std::fs::read_dir(source_parent).map_err(|_| Error::HostFailed)? {
        let entry = entry.map_err(|_| Error::HostFailed)?;
        if entry.path() == source {
            continue;
        }
        stage_link(&entry.path(), &staged_parent.join(entry.file_name()))
            .map_err(|_| Error::HostFailed)?;
    }
    let staged = staged_parent.join(source.file_name().ok_or(Error::InvalidBlueprint)?);
    std::fs::copy(&source, &staged).map_err(|_| Error::HostFailed)?;
    std::fs::set_permissions(&staged, metadata.permissions()).map_err(|_| Error::HostFailed)?;
    Ok((staged, directory))
}

fn staged_parent(directory: &Path, source_parent: &Path) -> Result<PathBuf> {
    if source_parent == Path::new("/") {
        return Ok(directory.to_path_buf());
    }
    Ok(directory.join(source_parent.file_name().ok_or(Error::InvalidBlueprint)?))
}

#[cfg(unix)]
fn stage_link(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}
#[cfg(windows)]
fn stage_link(source: &Path, target: &Path) -> std::io::Result<()> {
    if source.is_dir() {
        std::os::windows::fs::symlink_dir(source, target)
    } else {
        std::os::windows::fs::symlink_file(source, target)
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
