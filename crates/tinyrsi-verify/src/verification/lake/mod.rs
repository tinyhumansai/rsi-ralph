//! Immutable Lake staging, exact version checks and source-bound kernel diagnostics.
use crate::{CommandSpec, Error, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io,
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
///
/// # Errors
///
/// Returns [`Error::InvalidBlueprint`] for a command that is not a pinned Lake
/// build, and [`Error::HostFailed`] when the compiler or template cannot be
/// read.
pub fn identity(command: &CommandSpec) -> Result<serde_json::Value> {
    identity_with_compiler(command, None)
}

pub(crate) fn identity_with_compiler(
    command: &CommandSpec,
    compiler_override: Option<&Path>,
) -> Result<serde_json::Value> {
    let lean = compiler(command)?;
    let compiler = compiler_override.unwrap_or(&lean);
    let compiler_identity = lean.to_str().ok_or(Error::InvalidBlueprint)?;
    let metadata = std::fs::metadata(compiler).map_err(|_| Error::InvalidBlueprint)?;
    if !metadata.is_file() || !super::is_executable(&metadata) {
        return Err(Error::InvalidBlueprint);
    }
    let mut file = File::open(compiler).map_err(|_| Error::InvalidBlueprint)?;
    let mut hash = Sha256::new();
    io::copy(&mut file, &mut hash).map_err(|_| Error::HostFailed)?;
    Ok(
        serde_json::json!({"compiler": compiler_identity, "compiler_sha256": format!("{:x}", hash.finalize()),
        "project": PROJECT}),
    )
}

pub(crate) struct StagedToolchain {
    pub lake: PathBuf,
    pub lean: PathBuf,
    _directory: tempfile::TempDir,
}

/// Stages Lake and Lean while preserving the installed toolchain layout.
pub(crate) fn stage(command: &CommandSpec) -> Result<StagedToolchain> {
    let lake = std::fs::canonicalize(&command.program).map_err(|_| Error::InvalidBlueprint)?;
    let lean = compiler(command)?;
    let lake_metadata = std::fs::metadata(&lake).map_err(|_| Error::InvalidBlueprint)?;
    let lean_metadata = std::fs::metadata(&lean).map_err(|_| Error::InvalidBlueprint)?;
    if !lake_metadata.is_file()
        || !super::is_executable(&lake_metadata)
        || !lean_metadata.is_file()
        || !super::is_executable(&lean_metadata)
    {
        return Err(Error::InvalidBlueprint);
    }
    let source_bin = lake.parent().ok_or(Error::InvalidBlueprint)?;
    let in_bin_directory = source_bin.file_name().is_some_and(|name| name == "bin");
    let source_root = if in_bin_directory {
        source_bin.parent().ok_or(Error::InvalidBlueprint)?
    } else {
        source_bin
    };
    let directory = tempfile::tempdir().map_err(|_| Error::HostFailed)?;
    for entry in std::fs::read_dir(source_root).map_err(|_| Error::HostFailed)? {
        let entry = entry.map_err(|_| Error::HostFailed)?;
        if in_bin_directory && entry.file_name() == "bin" {
            continue;
        }
        link_path(&entry.path(), &directory.path().join(entry.file_name()))
            .map_err(|_| Error::HostFailed)?;
    }
    let staged_bin = if in_bin_directory {
        let bin = directory.path().join("bin");
        std::fs::create_dir(&bin).map_err(|_| Error::HostFailed)?;
        bin
    } else {
        directory.path().to_path_buf()
    };
    for entry in std::fs::read_dir(source_bin).map_err(|_| Error::HostFailed)? {
        let entry = entry.map_err(|_| Error::HostFailed)?;
        let source = entry.path();
        let target = staged_bin.join(entry.file_name());
        if source == lake {
            std::fs::copy(&source, &target).map_err(|_| Error::HostFailed)?;
            std::fs::set_permissions(&target, lake_metadata.permissions())
                .map_err(|_| Error::HostFailed)?;
        } else if source == lean {
            std::fs::copy(&source, &target).map_err(|_| Error::HostFailed)?;
            std::fs::set_permissions(&target, lean_metadata.permissions())
                .map_err(|_| Error::HostFailed)?;
        } else {
            link_path(&source, &target).map_err(|_| Error::HostFailed)?;
        }
    }
    let staged_lake = staged_bin.join(lake.file_name().ok_or(Error::InvalidBlueprint)?);
    let staged_lean = staged_bin.join(lean.file_name().ok_or(Error::InvalidBlueprint)?);
    Ok(StagedToolchain {
        lake: staged_lake,
        lean: staged_lean,
        _directory: directory,
    })
}

#[cfg(unix)]
fn link_path(source: &Path, target: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}

#[cfg(windows)]
fn link_path(source: &Path, target: &Path) -> io::Result<()> {
    if source.is_dir() {
        std::os::windows::fs::symlink_dir(source, target)
    } else {
        std::os::windows::fs::symlink_file(source, target)
    }
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
