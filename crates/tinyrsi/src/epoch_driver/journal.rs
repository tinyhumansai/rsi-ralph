//! Exclusive private generation journal and redacted restart receipts.
use super::{
    PreparedRetreatEpoch, RetreatEpochOutcome,
    types::{Entry, Snapshot},
};
use crate::{Error, GenerationMeter, Result};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
/// Durable epoch intent and result journal; generation outputs stay host-private.
pub struct EpochJournal {
    pub(super) path: PathBuf,
    pub(super) state: Snapshot,
    pub(super) writer_lock: std::sync::Arc<WriterLock>,
}
impl EpochJournal {
    /// Opens exclusively and validates already-completed generation receipts.
    /// # Errors
    /// Returns storage, locking or corrupt-journal errors.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let input = path.as_ref();
        let parent = input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent).map_err(|_| Error::ArchiveIo)?;
        let path = std::fs::canonicalize(parent)
            .map_err(|_| Error::ArchiveIo)?
            .join(input.file_name().ok_or(Error::ArchiveIo)?);
        let lockpath = path.with_extension("epoch-lock");
        if [&path, &lockpath]
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
            .open(lockpath)
            .map_err(|_| Error::ArchiveIo)?;
        fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| Error::ArchiveLocked)?;
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > 64 * 1024 * 1024) {
            return Err(Error::ArchiveCorrupt);
        }
        let state = match std::fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice::<Snapshot>(&bytes).map_err(|_| Error::ArchiveCorrupt)?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Snapshot {
                version: 1,
                ..Snapshot::default()
            },
            Err(_) => return Err(Error::ArchiveIo),
        };
        validate(&state)?;
        for entry in state.entries.values() {
            read_receipts(&path, entry)?;
        }
        Ok(Self {
            path,
            state,
            writer_lock: std::sync::Arc::new(WriterLock(lock)),
        })
    }
    /// Already prepared proposals; reading never repeats a model call.
    #[must_use]
    pub fn prepared(&self, epoch: u64) -> Option<&PreparedRetreatEpoch> {
        self.state
            .entries
            .get(&epoch)
            .and_then(|e| e.prepared.as_ref())
    }
    /// Durable public evaluation classifications.
    #[must_use]
    pub fn outcome(&self, epoch: u64) -> Option<&RetreatEpochOutcome> {
        self.state.entries.get(&epoch).map(|e| &e.outcome)
    }
    /// Accounting retained even when generation failed.
    #[must_use]
    pub fn generation_meter(&self, epoch: u64) -> Option<&GenerationMeter> {
        self.state.entries.get(&epoch).map(|e| &e.meter)
    }
    /// Schema-invalid generation attempts, including repaired proposals.
    #[must_use]
    pub fn generation_invalid_items(&self, epoch: u64) -> Option<usize> {
        self.state.entries.get(&epoch).map(|e| e.invalid_items)
    }
    /// Safe per-occurrence formatter receipts, including pending calls and failures.
    /// Returns `None` for an absent epoch or a legacy journal without receipts.
    /// # Errors
    /// Returns storage errors or corrupt receipt identity and ordering.
    pub fn generation_receipts(&self, epoch: u64) -> Result<Option<super::GenerationAudit>> {
        let Some(entry) = self.state.entries.get(&epoch) else {
            return Ok(None);
        };
        read_receipts(&self.path, entry)
    }
    pub(super) fn commit_entry(&mut self, entry: Entry) -> Result<()> {
        let mut next = self.state.clone();
        next.entries.insert(entry.epoch, entry);
        validate(&next)?;
        let bytes = serde_json::to_vec(&next).map_err(|_| Error::ArchiveIo)?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(Error::ArchiveIo);
        }
        let parent = self.path.parent().ok_or(Error::ArchiveIo)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|_| Error::ArchiveIo)?;
        temp.write_all(&bytes).map_err(|_| Error::ArchiveIo)?;
        temp.as_file().sync_all().map_err(|_| Error::ArchiveIo)?;
        temp.persist(&self.path).map_err(|_| Error::ArchiveIo)?;
        let result = File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::ArchiveIo);
        self.state = next;
        result
    }
}
fn validate(state: &Snapshot) -> Result<()> {
    if state.version != 1
        || state.entries.iter().any(|(id, e)| {
            *id != e.epoch
                || e.incumbent_id.is_empty()
                || (e.generation_receipts && e.generation_mode.is_none())
                || e.fingerprint.len() != 64
                || !e.fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
                || e.outcome.epoch != e.epoch
                || (e.evaluating && e.outcome.complete)
                || e.prepared.as_ref().is_some_and(|p| {
                    p.epoch != e.epoch
                        || p.incumbent_id != e.incumbent_id
                        || p.fingerprint != e.fingerprint
                        || p.ranked_items > p.proposals.len()
                        || p.proposals
                            .iter()
                            .any(|p| p.prediction.metric != "normalized_h")
                        || e.outcome.candidates.len() > p.proposals.len()
                })
        })
    {
        return Err(Error::ArchiveCorrupt);
    }
    Ok(())
}
impl std::fmt::Debug for EpochJournal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpochJournal")
            .field("epochs", &self.state.entries.len())
            .finish_non_exhaustive()
    }
}
pub(super) struct WriterLock(File);
impl Drop for WriterLock {
    fn drop(&mut self) {
        // Do not leave a lock held by a concurrent child's transient inherited fd.
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

fn read_receipts(path: &Path, entry: &Entry) -> Result<Option<super::GenerationAudit>> {
    let audit = super::receipts::read(path, entry.epoch, &entry.fingerprint)?;
    if (entry.generation_receipts && audit.is_none())
        || audit.as_ref().is_some_and(|audit| {
            entry.generation_mode.is_some_and(|mode| mode != audit.mode)
                || entry.prepared.as_ref().is_some_and(|prepared| {
                    audit.mode != prepared.mode
                        || audit
                            .calls
                            .iter()
                            .any(|call| call.status == super::GenerationCallStatus::Pending)
                })
        })
    {
        return Err(Error::ArchiveCorrupt);
    }
    Ok(audit)
}
