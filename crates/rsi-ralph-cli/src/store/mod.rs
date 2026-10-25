//! The durable run store under `$TINYRSI_HOME`.
//!
//! ```text
//! runs/<loop>/<run-id>/   loop/ run.json events.jsonl ledger.json epoch-<n>/
//! cache/<loop>/           incumbent score cache
//! spend.json              total spend across runs
//! retreat/<loop>/         creative-retreat memory
//! ```
//!
//! Every JSON document is written to a temporary sibling and renamed, so a
//! reader (the UI, `rsi-ralph runs`) never sees half a file.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Serialize, de::DeserializeOwned};

use crate::{Error, Result};

mod types;
pub use types::{RunRecord, RunStatus};

/// The run store rooted at one home folder.
#[derive(Debug, Clone)]
pub struct Store {
    home: PathBuf,
}

/// One run's folder.
#[derive(Debug, Clone)]
pub struct RunDir {
    /// The run's folder.
    pub path: PathBuf,
    /// Run id.
    pub id: String,
    /// Loop name.
    pub loop_name: String,
}

impl Store {
    /// The store at `home`.
    #[must_use]
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    /// `$TINYRSI_HOME`, else `.tinyrsi` under `$HOME` (or `%USERPROFILE%` on
    /// Windows, which sets no `HOME`), else `.tinyrsi`.
    #[must_use]
    pub fn default_home(lookup: &dyn Fn(&str) -> Option<String>) -> PathBuf {
        let set = |name: &str| lookup(name).filter(|value| !value.is_empty());
        let home = || {
            set("HOME").or_else(|| set("USERPROFILE")).map_or_else(
                || PathBuf::from(".tinyrsi"),
                |h| Path::new(&h).join(".tinyrsi"),
            )
        };
        set("TINYRSI_HOME").map_or_else(home, PathBuf::from)
    }

    /// The home folder.
    #[must_use]
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// The folder holding every run.
    #[must_use]
    pub fn runs_root(&self) -> PathBuf {
        self.home.join("runs")
    }

    /// The cross-run spend ledger.
    #[must_use]
    pub fn spend_ledger(&self) -> PathBuf {
        self.home.join("spend.json")
    }

    /// The incumbent score cache for a loop.
    #[must_use]
    pub fn cache_dir(&self, loop_name: &str) -> PathBuf {
        self.home.join("cache").join(loop_name)
    }

    /// The creative-retreat memory for a loop.
    #[must_use]
    pub fn retreat_dir(&self, loop_name: &str) -> PathBuf {
        self.home.join("retreat").join(loop_name)
    }

    /// Create a run folder and freeze a copy of the loop's files in it.
    ///
    /// Without an id, one is made from the current UTC time.
    ///
    /// # Errors
    /// Returns [`Error::RunExists`] when the id is taken, [`Error::Path`] for
    /// an id that is not a plain name, and [`Error::Io`] on write failures.
    pub fn create_run(
        &self,
        loop_name: &str,
        id: Option<&str>,
        files: &BTreeMap<String, String>,
    ) -> Result<RunDir> {
        let id = id.map_or_else(|| timestamp_id(now()), str::to_owned);
        if id.is_empty() || id.contains(['/', '\\']) || id.starts_with('.') {
            return Err(Error::Path { path: id });
        }
        let path = self.runs_root().join(loop_name).join(&id);
        if path.exists() {
            return Err(Error::RunExists(id));
        }
        let frozen = path.join("loop");
        for (relative, text) in files {
            let target = frozen.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| Error::io("create", parent, e))?;
            }
            fs::write(&target, text).map_err(|e| Error::io("write", &target, e))?;
        }
        fs::create_dir_all(&path).map_err(|e| Error::io("create", &path, e))?;
        Ok(RunDir {
            path,
            id,
            loop_name: loop_name.to_owned(),
        })
    }

    /// Every stored run, newest first, optionally for one loop.
    ///
    /// A run marked running whose process is gone is reported as
    /// [`RunStatus::Interrupted`].
    ///
    /// # Errors
    /// Returns [`Error::Io`] when the store cannot be listed; unreadable
    /// `run.json` files are skipped.
    pub fn list_runs(&self, loop_name: Option<&str>) -> Result<Vec<RunRecord>> {
        let root = self.runs_root();
        let mut runs = Vec::new();
        for loop_dir in children(&root)? {
            let name = file_name(&loop_dir);
            if loop_name.is_some_and(|wanted| wanted != name) {
                continue;
            }
            for run_dir in children(&loop_dir)? {
                if let Ok(record) = read_json::<RunRecord>(&run_dir.join("run.json")) {
                    runs.push(settle(record));
                }
            }
        }
        runs.sort_by(|a, b| b.started_at.cmp(&a.started_at).then(b.id.cmp(&a.id)));
        Ok(runs)
    }

    /// The folder and record of `<loop>/<run-id>`.
    ///
    /// # Errors
    /// Returns [`Error::NoRun`] when no such run exists.
    pub fn find_run(&self, reference: &str) -> Result<(RunDir, RunRecord)> {
        let (loop_name, id) = reference
            .split_once('/')
            .ok_or_else(|| Error::NoRun(reference.to_owned()))?;
        let path = self.runs_root().join(loop_name).join(id);
        let record = read_json::<RunRecord>(&path.join("run.json"))
            .map_err(|_| Error::NoRun(reference.to_owned()))?;
        let dir = RunDir {
            path,
            id: id.to_owned(),
            loop_name: loop_name.to_owned(),
        };
        Ok((dir, settle(record)))
    }
}

impl RunDir {
    /// `run.json`.
    #[must_use]
    pub fn record_path(&self) -> PathBuf {
        self.path.join("run.json")
    }

    /// `events.jsonl`.
    #[must_use]
    pub fn events_path(&self) -> PathBuf {
        self.path.join("events.jsonl")
    }

    /// `ledger.json`.
    #[must_use]
    pub fn ledger_path(&self) -> PathBuf {
        self.path.join("ledger.json")
    }

    /// The frozen loop folder.
    #[must_use]
    pub fn loop_dir(&self) -> PathBuf {
        self.path.join("loop")
    }

    /// `epoch-<n>/`, created on first use.
    ///
    /// # Errors
    /// Returns [`Error::Io`] when the folder cannot be created.
    pub fn epoch_dir(&self, epoch: u32) -> Result<PathBuf> {
        let dir = self.path.join(format!("epoch-{epoch}"));
        fs::create_dir_all(&dir).map_err(|e| Error::io("create", &dir, e))?;
        Ok(dir)
    }
}

/// Mark a dead running run as interrupted.
fn settle(mut record: RunRecord) -> RunRecord {
    if record.status == RunStatus::Running && !alive(record.pid) {
        record.status = RunStatus::Interrupted;
    }
    record
}

/// Whether a process exists, asked of `kill -0` so no unsafe code is needed.
fn alive(pid: u32) -> bool {
    pid == std::process::id()
        || Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
}

fn children(dir: &Path) -> Result<Vec<PathBuf>> {
    match fs::read_dir(dir) {
        Ok(entries) => Ok(entries
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(Error::io("list", dir, e)),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Current Unix time in seconds.
#[must_use]
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `YYYYMMDD-HHMMSS` in UTC for Unix seconds.
#[must_use]
pub fn timestamp_id(secs: u64) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Howard Hinnant's days-to-civil conversion for non-negative day counts.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    (y, m, d)
}

/// Write `value` as pretty JSON through a temporary sibling and a rename.
///
/// # Errors
/// Returns [`Error::Io`] when the file cannot be written.
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|e| Error::Store {
        path: path.to_owned(),
        message: e.to_string(),
    })?;
    write_atomic(path, text.as_bytes())
}

/// Write bytes through a temporary sibling and a rename.
///
/// # Errors
/// Returns [`Error::Io`] when the file cannot be written.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io("create", parent, e))?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| Error::io("write", &tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| Error::io("rename", path, e))
}

/// Read a JSON document.
///
/// # Errors
/// Returns [`Error::Io`] when it cannot be read and [`Error::Store`] when it
/// does not decode.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).map_err(|e| Error::io("read", path, e))?;
    serde_json::from_str(&text).map_err(|e| Error::Store {
        path: path.to_owned(),
        message: e.to_string(),
    })
}

/// Replace every occurrence of a secret value with `***`.
#[must_use]
pub fn scrub<'a>(text: &str, secrets: impl IntoIterator<Item = &'a str>) -> String {
    let mut out = text.to_owned();
    for secret in secrets {
        if secret.len() >= 4 {
            out = out.replace(secret, "***");
        }
    }
    out
}

#[cfg(test)]
mod test;
