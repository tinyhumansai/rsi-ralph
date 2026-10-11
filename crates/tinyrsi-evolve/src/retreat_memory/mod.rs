//! Durable, scoped rolling creative recall with separate host outcome windows.
//!
//! `DurableRetreatMemory` implements the tinybackrooms `Memory` trait over one
//! locked JSON file. It keeps a bounded window of discovery text for the
//! retreat Engine to recall and a separate window of host outcomes
//! (approved or rejected candidate IDs). It is a recall cache, not an audit
//! ledger; the archive remains authoritative. See this directory's
//! `README.md` for the eviction rule and canary checks.
mod types;
use crate::{Error, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tinybackrooms::{BoxFuture, Memory};
pub use types::*;
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    namespace: String,
    discovery: Vec<String>,
    outcomes: Vec<RetreatMemoryOutcome>,
}
/// Exclusive durable adapter for the real tinybackrooms memory port.
/// A stable OS lock prevents concurrent writers, including reopened instances.
pub struct DurableRetreatMemory {
    path: PathBuf,
    namespace: String,
    limits: MemoryLimits,
    forbidden: Vec<String>,
    state: Mutex<State>,
    writer_lock: File,
}
impl DurableRetreatMemory {
    /// Opens scoped durable history without overwriting corrupt state.
    /// # Errors
    /// Returns storage errors, a locked-store error or invalid configuration.
    pub fn open(
        path: impl AsRef<Path>,
        hive_id: &str,
        limits: MemoryLimits,
        forbidden: Vec<String>,
    ) -> Result<Self> {
        if hive_id.is_empty()
            || !hive_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            || limits.max_entries == 0
            || limits.max_bytes == 0
            || forbidden
                .iter()
                .any(|s| s.is_empty() || hive_id.contains(s))
        {
            return Err(Error::InvalidProposal);
        }
        let input = path.as_ref();
        let parent = input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent).map_err(|_| Error::ArchiveIo)?;
        let path = std::fs::canonicalize(parent)
            .map_err(|_| Error::ArchiveIo)?
            .join(input.file_name().ok_or(Error::ArchiveIo)?);
        if [&path, &path.with_extension("lock")]
            .iter()
            .any(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
        {
            return Err(Error::ArchiveIo);
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))
            .map_err(|_| Error::ArchiveIo)?;
        lock.try_lock_exclusive()
            .map_err(|_| Error::ArchiveLocked)?;
        let namespace = format!("project:rsi-{hive_id}/agent:retreat");
        let state = match std::fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice::<State>(&bytes).map_err(|_| Error::ArchiveCorrupt)?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State {
                version: 1,
                namespace: namespace.clone(),
                ..State::default()
            },
            Err(_) => return Err(Error::ArchiveIo),
        };
        let memory = Self {
            path,
            namespace,
            limits,
            forbidden,
            state: Mutex::new(state),
            writer_lock: lock,
        };
        {
            let state = memory.state.lock().map_err(|_| Error::HostFailed)?;
            memory.validate(&state)?;
        }
        Ok(memory)
    }
    fn validate(&self, state: &State) -> Result<()> {
        if state.version != 1
            || state.namespace != self.namespace
            || state.discovery.len() > self.limits.max_entries
            || total_bytes(&state.discovery, |s| Ok(s.len()))? > self.limits.max_bytes
            || total_bytes(&state.outcomes, outcome_bytes)? > self.limits.max_bytes
            || state.outcomes.len() > self.limits.max_entries
            || state.discovery.iter().any(|s| self.contaminated(s))
            || state.outcomes.iter().any(|s| {
                s.candidate_id.is_empty()
                    || s.candidate_id.len() > 256
                    || self.contaminated(&s.candidate_id)
            })
        {
            return Err(Error::ArchiveCorrupt);
        }
        Ok(())
    }
    fn trim<T>(&self, entries: &mut Vec<T>, size: impl Fn(&T) -> Result<usize>) -> Result<()> {
        let sizes = entries.iter().map(size).collect::<Result<Vec<_>>>()?;
        let mut bytes = sizes
            .iter()
            .try_fold(0usize, |a, b| a.checked_add(*b))
            .ok_or(Error::InvalidProposal)?;
        let mut removed = 0;
        while entries.len() - removed > self.limits.max_entries || bytes > self.limits.max_bytes {
            bytes -= sizes[removed];
            removed += 1;
        }
        entries.drain(..removed);
        Ok(())
    }
    fn persist(&self, current: &mut State, next: State) -> Result<()> {
        self.validate(&next)?;
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|_| Error::ArchiveIo)?;
        let bytes = serde_json::to_vec(&next).map_err(|_| Error::ArchiveIo)?;
        temp.write_all(&bytes).map_err(|_| Error::ArchiveIo)?;
        temp.as_file().sync_all().map_err(|_| Error::ArchiveIo)?;
        temp.persist(&self.path).map_err(|_| Error::ArchiveIo)?;
        // The rename is observable even when the following directory sync fails.
        *current = next;
        #[cfg(unix)]
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::ArchiveIo)?;
        Ok(())
    }
    /// Checks that public text contains no host-protected value.
    /// # Errors
    /// Returns invalid proposal when a protected string occurs.
    pub fn validate_public_text(&self, text: &str) -> Result<()> {
        if self.contaminated(text) {
            return Err(Error::InvalidProposal);
        }
        Ok(())
    }
    fn contaminated(&self, text: &str) -> bool {
        self.forbidden.iter().any(|f| text.contains(f))
            || serde_json::from_str::<serde_json::Value>(text)
                .is_ok_and(|value| self.contaminated_json(&value))
    }
    fn contaminated_json(&self, value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::String(text) => self.forbidden.iter().any(|f| text.contains(f)),
            serde_json::Value::Array(values) => values.iter().any(|v| self.contaminated_json(v)),
            serde_json::Value::Object(values) => values.iter().any(|(key, value)| {
                self.forbidden.iter().any(|f| key.contains(f)) || self.contaminated_json(value)
            }),
            _ => false,
        }
    }
    /// Exact authorized engine namespace.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    /// Records a host outcome in its own oldest-first bounded recall window.
    /// Repeated labels are idempotent, and contradictory labels reject, while
    /// the epoch/candidate pair remains retained. Archive/journal owns full history.
    /// # Errors
    /// Returns storage, bounds or protected-value errors.
    pub fn record_outcome(&self, outcome: RetreatMemoryOutcome) -> Result<()> {
        if outcome.candidate_id.is_empty()
            || outcome.candidate_id.len() > 256
            || self.contaminated(&outcome.candidate_id)
            || outcome_bytes(&outcome)? > self.limits.max_bytes
        {
            return Err(Error::InvalidProposal);
        }
        let mut state = self.state.lock().map_err(|_| Error::HostFailed)?;
        if state.outcomes.contains(&outcome) {
            return Ok(());
        }
        if state
            .outcomes
            .iter()
            .any(|e| e.epoch == outcome.epoch && e.candidate_id == outcome.candidate_id)
        {
            return Err(Error::InvalidProposal);
        }
        let mut next = State {
            version: state.version,
            namespace: state.namespace.clone(),
            discovery: state.discovery.clone(),
            outcomes: state.outcomes.clone(),
        };
        next.outcomes.push(outcome);
        self.trim(&mut next.outcomes, outcome_bytes)?;
        self.persist(&mut state, next)?;
        Ok(())
    }
    /// Returns only explicitly recorded host outcome labels.
    /// # Errors
    /// Returns an error if the host mutex is poisoned.
    pub fn outcomes(&self) -> Result<Vec<RetreatMemoryOutcome>> {
        Ok(self
            .state
            .lock()
            .map_err(|_| Error::HostFailed)?
            .outcomes
            .clone())
    }
}
impl Memory for DurableRetreatMemory {
    fn recall(
        &self,
        namespace: &str,
        _query: &str,
        limit: usize,
    ) -> BoxFuture<'_, tinybackrooms::Result<Vec<String>>> {
        let result = (|| {
            if namespace != self.namespace {
                return Err(Error::InvalidProposal);
            }
            let state = self.state.lock().map_err(|_| Error::HostFailed)?;
            Ok(state.discovery[state.discovery.len().saturating_sub(limit)..].to_vec())
        })();
        Box::pin(async move {
            result.map_err(|_| tinybackrooms::Error::Memory("durable recall failed".into()))
        })
    }
    fn store(
        &self,
        namespace: &str,
        entries: Vec<String>,
    ) -> BoxFuture<'_, tinybackrooms::Result<()>> {
        let result = (|| {
            if namespace != self.namespace {
                return Err(Error::InvalidProposal);
            }
            // Check every incoming item before eviction, even items that would
            // fall outside the retained suffix of a larger batch.
            if entries
                .iter()
                .any(|s| self.contaminated(s) || s.len() > self.limits.max_bytes)
            {
                return Err(Error::InvalidProposal);
            }
            let mut state = self.state.lock().map_err(|_| Error::HostFailed)?;
            let mut next = State {
                version: state.version,
                namespace: state.namespace.clone(),
                discovery: state.discovery.clone(),
                outcomes: state.outcomes.clone(),
            };
            next.discovery.extend(entries);
            self.trim(&mut next.discovery, |s| Ok(s.len()))?;
            self.persist(&mut state, next)?;
            Ok(())
        })();
        Box::pin(async move {
            result.map_err(|_| tinybackrooms::Error::Memory("durable store failed".into()))
        })
    }
}
fn outcome_bytes(outcome: &RetreatMemoryOutcome) -> Result<usize> {
    serde_json::to_vec(outcome)
        .map(|bytes| bytes.len())
        .map_err(|_| Error::InvalidProposal)
}
fn total_bytes<T>(entries: &[T], size: impl Fn(&T) -> Result<usize>) -> Result<usize> {
    entries.iter().try_fold(0usize, |total, entry| {
        total.checked_add(size(entry)?).ok_or(Error::ArchiveCorrupt)
    })
}
#[cfg(test)]
mod test;

impl std::fmt::Debug for DurableRetreatMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DurableRetreatMemory")
            .field("namespace", &self.namespace)
            .finish_non_exhaustive()
    }
}
impl Drop for DurableRetreatMemory {
    fn drop(&mut self) {
        // Explicit unlock also releases a descriptor briefly inherited by a
        // concurrently spawning process before its close-on-exec runs.
        let _ = fs2::FileExt::unlock(&self.writer_lock);
    }
}
