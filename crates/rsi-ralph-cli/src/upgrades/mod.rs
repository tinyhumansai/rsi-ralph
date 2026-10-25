//! The upgrade log: why a loop's hive package or config changed between
//! runs, what the change should do, and whether it did.
//!
//! The log is a set of `upgrades.jsonl` files, one [`Entry`] per line, and
//! their union is the source of truth:
//!
//! - `<loop>/upgrades.jsonl` holds what people record: the operator's and
//!   their assistant's [`Upgrade`]s and [`Outcome`]s, written by
//!   `rsi-ralph upgrades add` and `rsi-ralph upgrades outcome`. It is part of
//!   the loop folder, so every run freezes a copy of it.
//! - `runs/<loop>/<run>/upgrades.jsonl` holds what a run records about
//!   itself: the [`Applied`] entry written when it starts (what differs from
//!   the previous run, and which upgrades explain it) and an unexplained
//!   [`Upgrade`] for any difference no record names. [`record`] is the one
//!   writer, and the hook for any later mechanism that lets a hive change
//!   itself.
//!
//! [`render::markdown`] turns the union into the Markdown `upgrades show`
//! prints and each run stores as `upgrades.md`. The format is specified in
//! [`docs/specs/upgrade-log.md`](https://github.com/tinyhumansai/rsi-ralph/blob/main/docs/specs/upgrade-log.md).

use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use sha2::{Digest, Sha256};

use crate::{
    Error, Result,
    store::{RunDir, Store},
};

pub mod drift;
pub mod notation;
pub mod render;
mod types;
pub use types::{
    Applied, AppliesFrom, Author, Change, Comparison, Contributor, Entry, Evidence, FileChange,
    Outcome, Prediction, Role, Upgrade, Verdict,
};

/// The log's file name, in a loop folder and in a run folder.
pub const LOG: &str = "upgrades.jsonl";

/// The Markdown rendering a run stores beside its log.
pub const RENDERED: &str = "upgrades.md";

/// Parse the text of an `upgrades.jsonl`; blank lines are skipped.
///
/// # Errors
/// Returns [`Error::Store`] naming `origin` and the line that does not
/// decode.
pub fn parse(text: &str, origin: &Path) -> Result<Vec<Entry>> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            serde_json::from_str(line).map_err(|e| Error::Store {
                path: origin.to_owned(),
                message: format!("line {}: {e}", i + 1),
            })
        })
        .collect()
}

/// Read an `upgrades.jsonl`; a missing file is an empty log.
///
/// # Errors
/// Returns [`Error::Io`] when it cannot be read and [`Error::Store`] for a
/// line that does not decode.
pub fn read(path: &Path) -> Result<Vec<Entry>> {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text, path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(Error::io("read", path, e)),
    }
}

/// Append one entry to an `upgrades.jsonl`, creating it if needed.
///
/// # Errors
/// Returns [`Error::Io`] when the file cannot be written.
pub fn append(path: &Path, entry: &Entry) -> Result<()> {
    let mut line = serde_json::to_string(entry).map_err(|e| Error::Store {
        path: path.to_owned(),
        message: e.to_string(),
    })?;
    line.push('\n');
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io("create", parent, e))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(line.as_bytes()))
        .map_err(|e| Error::io("append", path, e))
}

/// Record a change a run made to its own hive or loop, in the run's log.
///
/// This is the writer for every change the loop or the hive makes to itself.
/// Today only the run-start diff calls it; a mechanism that lets a seat amend
/// the hive must call it with the seat as a [`Author::Hive`] contributor and
/// the seat's own reason as the rationale.
///
/// # Errors
/// Returns [`Error::Io`] when the run's log cannot be written.
pub fn record(run: &RunDir, upgrade: Upgrade) -> Result<()> {
    append(&run.path.join(LOG), &Entry::Upgrade(upgrade))
}

/// Every entry of a loop's log: `loop_log` (the loop folder's) followed by
/// each stored run's own, oldest run first. Runs are matched by loop name.
///
/// # Errors
/// Returns [`Error::Io`] or [`Error::Store`] when a run's log cannot be
/// read.
pub fn collect(store: &Store, loop_name: &str, loop_log: Vec<Entry>) -> Result<Vec<Entry>> {
    let mut entries = loop_log;
    let mut runs = store.list_runs(Some(loop_name))?;
    runs.reverse();
    for run in runs {
        let path = store.runs_root().join(loop_name).join(&run.id).join(LOG);
        entries.extend(read(&path)?);
    }
    Ok(entries)
}

/// The loop folder's log as the loop's latest stored run froze it; `None`
/// when the loop has no stored run.
///
/// # Errors
/// Returns [`Error::Io`] when the store cannot be read and [`Error::Store`]
/// for a frozen log that does not decode.
pub fn frozen_log(store: &Store, loop_name: &str) -> Result<Option<Vec<Entry>>> {
    let Some((_, files)) = drift::previous_run(store, loop_name, "")? else {
        return Ok(None);
    };
    files
        .get(LOG)
        .map_or_else(|| Ok(Vec::new()), |text| parse(text, Path::new(LOG)))
        .map(Some)
}

/// The upgrades among `entries`.
pub fn upgrades(entries: &[Entry]) -> impl Iterator<Item = &Upgrade> {
    entries.iter().filter_map(|e| match e {
        Entry::Upgrade(u) => Some(u),
        _ => None,
    })
}

/// A fresh upgrade id for `now`, `u-<YYYYMMDD-HHMMSS>`, suffixed `-2`,
/// `-3`, … when that id is taken.
#[must_use]
pub fn next_id(entries: &[Entry], now: u64) -> String {
    let taken: BTreeSet<&str> = upgrades(entries).map(|u| u.id.as_str()).collect();
    let base = format!("u-{}", crate::store::timestamp_id(now));
    if !taken.contains(base.as_str()) {
        return base;
    }
    // Among `taken.len() + 1` candidates at least one is free.
    (2..=taken.len() + 2)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken.contains(id.as_str()))
        .unwrap_or(base)
}

/// SHA-256 of `text`, hex.
#[must_use]
pub fn digest(text: &str) -> String {
    crate::subject::hex(&Sha256::digest(text.as_bytes()))
}

impl Comparison {
    /// Whether `measured` satisfies `measured op threshold`.
    #[must_use]
    pub fn holds(self, measured: f64, threshold: f64) -> bool {
        match self {
            Self::Gt => measured > threshold,
            Self::Ge => measured >= threshold,
            Self::Lt => measured < threshold,
            Self::Le => measured <= threshold,
            Self::Eq => (measured - threshold).abs() <= 1e-9,
        }
    }

    /// The operator as written, such as `>=`.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Eq => "=",
        }
    }
}

/// The measured value of `metric` in `measured`, if any.
#[must_use]
pub fn measured_value(measured: &[Evidence], metric: &str) -> Option<f64> {
    measured
        .iter()
        .find(|e| e.metric.as_deref() == Some(metric))
        .and_then(|e| e.value)
}

/// The verdict `measured` gives `expected`: confirmed when every measured
/// prediction held, refuted when none did, mixed otherwise. `None` when no
/// prediction was measured.
#[must_use]
pub fn evaluate(expected: &[Prediction], measured: &[Evidence]) -> Option<Verdict> {
    let results: Vec<bool> = expected
        .iter()
        .filter_map(|p| measured_value(measured, &p.metric).map(|v| p.op.holds(v, p.value)))
        .collect();
    if results.is_empty() {
        None
    } else if results.iter().all(|held| *held) {
        Some(Verdict::Confirmed)
    } else if results.iter().any(|held| *held) {
        Some(Verdict::Mixed)
    } else {
        Some(Verdict::Refuted)
    }
}

#[cfg(test)]
mod test;
