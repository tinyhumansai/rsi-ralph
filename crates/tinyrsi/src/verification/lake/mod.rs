//! Immutable Lake staging, exact version checks and source-bound kernel diagnostics.
use crate::{CommandSpec, Error, Result};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

const PROJECT: &str = "name = \"rsi_verified\"\nversion = \"0.0.0\"\ndefaultTargets = [\"Rsi\"]\n\n[[lean_lib]]\nname = \"Rsi\"\n";

/// Resolve neighboring actual compiler binaries, rejecting Elan dispatch shims.
fn compiler(command: &CommandSpec) -> Result<PathBuf> {
    let lake = std::fs::canonicalize(&command.program).map_err(|_| Error::InvalidBlueprint)?;
    if !Path::new(&command.program).is_absolute() || lake.file_name().is_none_or(|n| n != "lake") {
        return Err(Error::InvalidBlueprint);
    }
    let parent = lake.parent().ok_or(Error::InvalidBlueprint)?;
    let lean = std::fs::canonicalize(parent.join("lean")).map_err(|_| Error::InvalidBlueprint)?;
    if lean.parent() != Some(parent) || lean.file_name().is_none_or(|n| n != "lean") {
        return Err(Error::InvalidBlueprint);
    }
    Ok(lean)
}

/// Require an exact stable toolchain version and frozen project command.
pub(crate) fn validate(command: &CommandSpec, theorem: &str, toolchain: &str) -> Result<()> {
    super::validate_command(command)?;
    let version = version(toolchain)?;
    if command.args != ["build"]
        || !theorem.contains("theorem rsi_goal")
        || !theorem.trim_end().ends_with(":=")
        || version.is_empty()
    {
        return Err(Error::InvalidBlueprint);
    }
    compiler(command)?;
    Ok(())
}

/// Parse a stable identity without permitting ranges, aliases or whitespace.
fn version(toolchain: &str) -> Result<&str> {
    let version = toolchain
        .strip_prefix("leanprover/lean4:v")
        .ok_or(Error::InvalidBlueprint)?;
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(Error::InvalidBlueprint);
    }
    Ok(version)
}

/// Bind the installed compiler bytes and immutable project template into suite identity.
pub(crate) fn identity(command: &CommandSpec) -> Result<serde_json::Value> {
    let lean = compiler(command)?;
    let bytes = std::fs::read(&lean).map_err(|_| Error::HostFailed)?;
    Ok(
        serde_json::json!({"compiler": lean, "compiler_sha256": format!("{:x}", Sha256::digest(bytes)),
        "project": PROJECT}),
    )
}

/// Observe the pinned version, then build under the remaining aggregate limits.
pub(crate) fn execute(
    command: &CommandSpec,
    source: &str,
    toolchain: &str,
) -> Result<(bool, Option<i32>)> {
    let started = Instant::now();
    let mut probe = command.clone();
    probe.program = compiler(command)?.to_string_lossy().into_owned();
    probe.args = vec!["--version".into()];
    let (passed, _, output) = super::process(&probe, b"")?;
    let prefix = format!("Lean (version {},", version(toolchain)?);
    if !passed
        || !std::str::from_utf8(&output)
            .is_ok_and(|s| s.starts_with(&prefix) && s.lines().count() == 1)
    {
        return Ok((false, None));
    }
    let elapsed = u64::try_from(started.elapsed().as_millis()).map_err(|_| Error::HostFailed)?;
    let mut build = command.clone();
    build.timeout_ms = command.timeout_ms.saturating_sub(elapsed);
    build.max_output_bytes = command.max_output_bytes.saturating_sub(output.len());
    if build.timeout_ms == 0 || build.max_output_bytes == 0 {
        return Ok((false, None));
    }
    let (passed, code, output) = super::process_with_files(
        &build,
        source.as_bytes(),
        &[
            ("lakefile.toml", PROJECT.as_bytes()),
            ("lean-toolchain", toolchain.as_bytes()),
            ("Rsi.lean", source.as_bytes()),
        ],
    )?;
    Ok((passed && super::trusted_axioms(&output, true), code))
}

/// Accept only a numeric source location in the actual frozen root file.
pub(crate) fn diagnostic(line: &str) -> Option<&str> {
    let (location, message) = line.strip_prefix("info: Rsi.lean:")?.split_once(": ")?;
    let (row, col) = location.split_once(':')?;
    if row.parse::<u32>().ok()? == 0 || col.parse::<u32>().is_err() {
        return None;
    }
    Some(message)
}
