//! Candidate materialization, bounded per-seed execution and metric parsing.
use super::{ScriptSubject, SeedRun};
use crate::{Error, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tinyrsi_evolve::{ScriptProposal, is_plain_relative};
use tinyrsi_verify::process::{self, Exit, ProcessSpec};

/// Editable file contents keyed by workspace-relative path.
pub(super) type Sources = BTreeMap<PathBuf, String>;

/// Bytes of stderr kept in the ledger and shown to the supervisor.
const STDERR_TAIL: usize = 2_000;

/// `DIAG` lines kept per seed; the last ones win, since a script's summary
/// usually comes at the end.
pub(super) const DIAG_LINES: usize = 20;

/// Characters kept per `DIAG` line before it is cut with an ellipsis.
pub(super) const DIAG_CHARS: usize = 300;

/// The bounded `DIAG <text>` lines of script stdout, in order.
///
/// Diagnostics are free text for the supervisor: what the script observed or
/// what the code under improvement did (moves tried, conflicts left, residual
/// shape). Empty lines are dropped; at most the last [`DIAG_LINES`] are kept,
/// each cut to [`DIAG_CHARS`] characters.
pub(super) fn parse_diagnostics(stdout: &str) -> Vec<String> {
    let lines: Vec<String> = stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix("DIAG "))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| {
            if text.chars().count() > DIAG_CHARS {
                let mut cut: String = text.chars().take(DIAG_CHARS).collect();
                cut.push('…');
                cut
            } else {
                text.to_owned()
            }
        })
        .collect();
    let skip = lines.len().saturating_sub(DIAG_LINES);
    lines.into_iter().skip(skip).collect()
}

/// Parses the last `METRIC <name>=<value>` line for `name` from script stdout.
///
/// Other lines and other metric names are ignored, so a script may print
/// progress and several metrics.
///
/// # Errors
///
/// Returns [`Error::MetricMissing`] when no line names the metric and
/// [`Error::MetricInvalid`] when the last such value is not a finite number.
///
/// # Examples
///
/// ```
/// let stdout = "fitting...\nMETRIC rmse=0.25\n";
/// assert_eq!(tinyrsi::parse_metric(stdout, "rmse")?, 0.25);
/// # Ok::<(), tinyrsi::Error>(())
/// ```
pub fn parse_metric(stdout: &str, name: &str) -> Result<f64> {
    let value = stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix("METRIC "))
        .filter_map(|rest| rest.split_once('='))
        .filter(|(key, _)| key.trim() == name)
        .map(|(_, value)| value.trim())
        .next_back()
        .ok_or(Error::MetricMissing)?;
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or(Error::MetricInvalid)
}

/// The incumbent's sources with `proposal`'s files written over them.
pub(super) fn apply(proposal: &ScriptProposal, incumbent: &Sources) -> Sources {
    let mut next = incumbent.clone();
    for file in &proposal.files {
        next.insert(PathBuf::from(&file.path), file.content.clone());
    }
    next
}

/// Checks the subject before any file is touched.
pub(super) fn validate_subject(subject: &ScriptSubject) -> Result<()> {
    let metric = &subject.metric.name;
    let distinct: BTreeSet<_> = subject.seeds.iter().collect();
    let editable: BTreeSet<_> = subject.editable.iter().collect();
    if subject.name.trim().is_empty()
        || metric.is_empty()
        || metric.contains(['=', ' ', '\t', '\n'])
        || subject.metric.target.is_some_and(|t| !t.is_finite())
        || !subject.workspace.is_dir()
        || subject.editable.is_empty()
        || editable.len() != subject.editable.len()
        || subject.editable.iter().any(|p| !is_plain_relative(p))
        || subject.seeds.len() < 2
        || distinct.len() != subject.seeds.len()
        || subject.timeout_ms == 0
        || subject.output_cap == 0
        || subject.command.program.as_os_str().is_empty()
        || subject
            .env
            .iter()
            .any(|(k, _)| k.is_empty() || k.contains('=') || k == "TINYRSI_SEED")
    {
        return Err(Error::InvalidScriptSubject);
    }
    Ok(())
}

/// Reads the editable files of the workspace.
pub(super) fn read_sources(subject: &ScriptSubject) -> Result<Sources> {
    subject
        .editable
        .iter()
        .map(|path| {
            fs::read_to_string(subject.workspace.join(path))
                .map(|text| (path.clone(), text))
                .map_err(|_| Error::InvalidScriptSubject)
        })
        .collect()
}

/// Short content id: the first 12 hex digits of SHA-256 over the sources.
pub(super) fn content_id(sources: &Sources) -> String {
    let mut hash = Sha256::new();
    for (path, text) in sources {
        let path = path.to_string_lossy();
        // Length prefixes keep `(a, bc)` and `(ab, c)` from colliding.
        hash.update((path.len() as u64).to_le_bytes());
        hash.update(path.as_bytes());
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    }
    format!("{:x}", hash.finalize())[..12].to_owned()
}

/// Copies the workspace into `dest` and writes `sources` over it.
///
/// Symlinks and `skip` (the run directory, when it lives in the workspace) are
/// not copied.
pub(super) fn materialize(
    workspace: &Path,
    skip: &Path,
    dest: &Path,
    sources: &Sources,
) -> Result<()> {
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|_| Error::HostFailed)?;
    }
    copy_tree(workspace, skip, dest)?;
    for (path, text) in sources {
        let target = dest.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|_| Error::HostFailed)?;
        }
        fs::write(target, text).map_err(|_| Error::HostFailed)?;
    }
    Ok(())
}

fn copy_tree(from: &Path, skip: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).map_err(|_| Error::HostFailed)?;
    for entry in fs::read_dir(from).map_err(|_| Error::HostFailed)? {
        let entry = entry.map_err(|_| Error::HostFailed)?;
        let kind = entry.file_type().map_err(|_| Error::HostFailed)?;
        let source = entry.path();
        if source == skip || kind.is_symlink() {
            continue;
        }
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&source, skip, &target)?;
        } else {
            fs::copy(&source, &target).map_err(|_| Error::HostFailed)?;
        }
    }
    Ok(())
}

/// Runs the subject's command once in `dir` with `seed`.
///
/// Every failure becomes a failed [`SeedRun`] rather than an error, so one bad
/// seed never aborts the loop.
pub(super) fn run_seed(subject: &ScriptSubject, dir: &Path, tmp_root: &Path, seed: u64) -> SeedRun {
    match execute(subject, dir, tmp_root, seed) {
        Ok((result, duration_ms, stderr, diagnostics)) => {
            let tail = stderr.len().saturating_sub(STDERR_TAIL);
            let stderr_tail = String::from_utf8_lossy(&stderr[tail..]).into_owned();
            let (metric, error) = match result {
                Ok(metric) => (Some(metric), None),
                Err(error) => (None, Some(error.to_string())),
            };
            SeedRun {
                seed,
                metric,
                error,
                duration_ms,
                stderr_tail,
                diagnostics,
            }
        }
        Err(error) => SeedRun {
            seed,
            metric: None,
            error: Some(error.to_string()),
            duration_ms: 0,
            stderr_tail: String::new(),
            diagnostics: Vec::new(),
        },
    }
}

/// The metric (or why there is none), wall time, raw stderr and diagnostics.
type Execution = (Result<f64>, u64, Vec<u8>, Vec<String>);

fn execute(subject: &ScriptSubject, dir: &Path, tmp_root: &Path, seed: u64) -> Result<Execution> {
    fs::create_dir_all(tmp_root).map_err(|_| Error::HostFailed)?;
    let capture = tempfile::tempdir_in(tmp_root).map_err(|_| Error::HostFailed)?;
    let tmp = capture.path().join("tmp");
    fs::create_dir(&tmp).map_err(|_| Error::HostFailed)?;
    let mut env = vec![
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ("TMPDIR".to_owned(), tmp.to_string_lossy().into_owned()),
    ];
    env.extend(subject.env.iter().cloned());
    env.push(("TINYRSI_SEED".to_owned(), seed.to_string()));
    let outcome = process::run(&ProcessSpec {
        program: subject.command.program.as_os_str(),
        args: &subject.command.args,
        cwd: dir,
        scratch: capture.path(),
        env: &env,
        timeout: Duration::from_millis(subject.timeout_ms),
        cap: subject.output_cap,
    })?;
    let total = outcome.stdout.len().saturating_add(outcome.stderr.len());
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    let diagnostics = parse_diagnostics(&stdout);
    let result = match outcome.exit {
        Exit::TimedOut => Err(Error::ScriptTimeout),
        Exit::OutputExceeded => Err(Error::ScriptOutputExceeded),
        Exit::Exited { .. } if u64::try_from(total).map_or(true, |n| n > subject.output_cap) => {
            Err(Error::ScriptOutputExceeded)
        }
        Exit::Exited { success: false, .. } => Err(Error::ScriptFailed),
        Exit::Exited { success: true, .. } => parse_metric(&stdout, &subject.metric.name),
    };
    Ok((result, outcome.duration_ms, outcome.stderr, diagnostics))
}
