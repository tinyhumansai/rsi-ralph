//! The run-start diff: compare a run's loop folder with the previous run's,
//! so no change to the hive package or the loop config goes unrecorded.
//!
//! Every file of the loop folder counts except the upgrade log itself and,
//! for a `dir` subject inside the loop folder, the subject: promotion changes
//! the subject by design, and its history is the run ledger's. A changed file
//! is explained when an upgrade this run is the first to carry names it (or
//! a folder above it, written with a trailing `/`). Upgrades a run carries
//! first are those in its loop log that the previous run's frozen log did not
//! hold, plus any whose `applies_from.run` names it.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    Applied, Author, Change, Contributor, Entry, FileChange, LOG, RENDERED, Role, Upgrade, append,
    collect, digest, parse, render, upgrades,
};
use crate::{
    Result,
    manifest::{self, Loop, SubjectKind},
    store::{self, RunDir, Store},
};

/// The mechanism name unexplained-change records carry.
pub const MECHANISM: &str = "run_start_diff";

/// Files that differ between `previous` and `current`, each with its
/// before and after hash, skipping paths under any of `excluded` (each a
/// file name or a folder ending in `/`).
#[must_use]
pub fn compare(
    previous: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
    excluded: &[String],
) -> Vec<FileChange> {
    let skip = |path: &str| {
        excluded
            .iter()
            .any(|e| path == e || (e.ends_with('/') && path.starts_with(e.as_str())))
    };
    let paths: BTreeSet<&String> = previous.keys().chain(current.keys()).collect();
    paths
        .into_iter()
        .filter(|path| !skip(path))
        .filter_map(|path| {
            let before = previous.get(path);
            let after = current.get(path);
            (before != after).then(|| FileChange {
                path: path.clone(),
                keys: Vec::new(),
                before: before.map(|t| digest(t)),
                after: after.map(|t| digest(t)),
            })
        })
        .collect()
}

/// Whether `named` (a file, or a folder ending in `/`) covers `path`.
#[must_use]
pub fn covers(named: &str, path: &str) -> bool {
    named == path || (named.ends_with('/') && path.starts_with(named))
}

/// One line per changed file: added, removed, or line counts before and
/// after.
#[must_use]
pub fn summary(
    changes: &[FileChange],
    previous: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
) -> String {
    changes
        .iter()
        .map(|c| {
            let lines = |files: &BTreeMap<String, String>| files.get(&c.path).map(|t| t.lines().count());
            match (lines(previous), lines(current)) {
                (None, Some(n)) => format!("{}: added ({n} lines)", c.path),
                (Some(n), None) => format!("{}: removed ({n} lines)", c.path),
                (Some(a), Some(b)) => format!("{}: modified ({a} -> {b} lines)", c.path),
                (None, None) => c.path.clone(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The paths the run-start diff skips for `parsed`: the log, and a `dir`
/// subject's folder.
#[must_use]
pub fn excluded(parsed: &Loop) -> Vec<String> {
    let mut out = vec![LOG.to_owned()];
    let subject = &parsed.manifest.subject;
    if subject.kind == SubjectKind::Dir {
        let path = subject.path.trim_start_matches("./").trim_end_matches('/');
        if !path.is_empty() && path != "." {
            out.push(format!("{path}/"));
        }
    }
    out
}

/// The loop's most recent stored run other than `run`, with its frozen loop
/// files; `None` when there is none.
///
/// # Errors
/// Returns [`crate::Error::Io`] when the store or the frozen copy cannot be
/// read.
pub fn previous_run(
    store: &Store,
    loop_name: &str,
    run: &str,
) -> Result<Option<(String, BTreeMap<String, String>)>> {
    for record in store.list_runs(Some(loop_name))? {
        if record.id == run {
            continue;
        }
        let frozen = store.runs_root().join(loop_name).join(&record.id).join("loop");
        if frozen.is_dir() {
            return Ok(Some((record.id, manifest::load_dir(&frozen)?)));
        }
    }
    Ok(None)
}

/// Compare the run's loop files with the previous run's and record the
/// result in the run's log: always an [`Applied`] entry, and an unexplained
/// [`Upgrade`] when some change is named by no upgrade. Then store the
/// rendered log as the run's `upgrades.md`.
///
/// Returns a warning for `run.json` when something changed unexplained.
///
/// # Errors
/// Returns [`crate::Error::Store`] for a loop log that does not decode, and
/// [`crate::Error::Io`] when the store cannot be read or written.
pub fn check_run_start(
    store: &Store,
    run: &RunDir,
    files: &BTreeMap<String, String>,
    parsed: &Loop,
    now: u64,
) -> Result<Option<String>> {
    let log_path = run.loop_dir().join(LOG);
    let current_log = files
        .get(LOG)
        .map_or_else(|| Ok(Vec::new()), |text| parse(text, &log_path))?;
    let previous = previous_run(store, &run.loop_name, &run.id)?;
    let (previous_id, previous_files) = previous.map_or((None, BTreeMap::new()), |(id, f)| (Some(id), f));
    let carried_before: BTreeSet<String> = match previous_files.get(LOG) {
        Some(text) => upgrades(&parse(text, &log_path)?).map(|u| u.id.clone()).collect(),
        None => BTreeSet::new(),
    };
    let carried: Vec<&Upgrade> = upgrades(&current_log)
        .filter(|u| {
            u.applies_from.run.as_deref() == Some(run.id.as_str())
                || (u.applies_from.run.is_none() && !carried_before.contains(&u.id))
        })
        .collect();
    let changes = if previous_id.is_some() {
        compare(&previous_files, files, &excluded(parsed))
    } else {
        Vec::new()
    };
    let unexplained: Vec<FileChange> = changes
        .iter()
        .filter(|c| {
            !carried
                .iter()
                .any(|u| u.change.files.iter().any(|f| covers(&f.path, &c.path)))
        })
        .cloned()
        .collect();
    let applied = Applied {
        run: run.id.clone(),
        recorded_at: now,
        previous_run: previous_id.clone(),
        upgrades: carried.iter().map(|u| u.id.clone()).collect(),
        files: changes,
        unexplained: unexplained.iter().map(|c| c.path.clone()).collect(),
    };
    append(&run.path.join(LOG), &Entry::Applied(applied))?;
    let warning = if unexplained.is_empty() {
        None
    } else {
        let previous_id = previous_id.unwrap_or_default();
        let paths: Vec<&str> = unexplained.iter().map(|c| c.path.as_str()).collect();
        let diff = summary(&unexplained, &previous_files, files);
        super::record(
            run,
            Upgrade {
                id: format!("u-{}-unexplained", run.id),
                recorded_at: now,
                title: format!("Unexplained change since run {previous_id}"),
                contributors: vec![Contributor {
                    author: Author::Loop {
                        mechanism: MECHANISM.to_owned(),
                    },
                    role: Role::Detected,
                    rationale: Some(format!(
                        "{} changed since run {previous_id} and no upgrade record names {}",
                        paths.len(),
                        if paths.len() == 1 { "it" } else { "them" }
                    )),
                }],
                applies_from: super::AppliesFrom {
                    run: Some(run.id.clone()),
                    epoch: None,
                },
                evidence: Vec::new(),
                diagnosis: None,
                change: Change {
                    summary: format!("Files changed with no upgrade record: {}", paths.join(", ")),
                    files: unexplained,
                    diff: Some(diff),
                },
                expected: Vec::new(),
            },
        )?;
        Some(format!(
            "changed since run {previous_id} with no upgrade record: {}",
            paths.join(", ")
        ))
    };
    let entries = collect(store, &run.loop_name, current_log)?;
    let text = render::markdown(&run.loop_name, &entries);
    store::write_atomic(&run.path.join(RENDERED), text.as_bytes())?;
    Ok(warning)
}
