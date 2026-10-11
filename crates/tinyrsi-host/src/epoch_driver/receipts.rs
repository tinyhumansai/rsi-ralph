//! Atomic redacted sidecars under the parent epoch journal's writer lock.
use super::{
    GenerationAudit, GenerationCallReceipt, GenerationCallStatus, GenerationDisposition,
    GenerationMode,
};
use crate::{Error, Result};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
const MAX_BYTES: u64 = 64 * 1024 * 1024;
pub(super) struct ReceiptStore {
    path: PathBuf,
    _writer: Arc<super::journal::WriterLock>,
    state: Mutex<(GenerationAudit, bool)>,
}
pub(super) fn path(journal: &Path, epoch: u64) -> PathBuf {
    let mut name = journal.as_os_str().to_owned();
    name.push(format!(".generation-{epoch}.json"));
    PathBuf::from(name)
}
fn safe_path(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => Err(Error::ArchiveIo),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(Error::ArchiveIo),
    }
}
pub(super) fn read(
    journal: &Path,
    epoch: u64,
    fingerprint: &str,
) -> Result<Option<GenerationAudit>> {
    let path = path(journal, epoch);
    safe_path(&path)?;
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        return Err(Error::ArchiveCorrupt);
    }
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::ArchiveIo),
    };
    let audit: GenerationAudit =
        serde_json::from_slice(&bytes).map_err(|_| Error::ArchiveCorrupt)?;
    if audit.version != 1
        || audit.epoch != epoch
        || audit.fingerprint != fingerprint
        || audit.calls.iter().zip(1_u64..).any(|(call, id)| {
            call.id != id
                || (call.status != GenerationCallStatus::Returned && !call.items.is_empty())
        })
    {
        return Err(Error::ArchiveCorrupt);
    }
    Ok(Some(audit))
}
impl ReceiptStore {
    pub(super) fn create(
        journal: &Path,
        epoch: u64,
        fingerprint: String,
        mode: GenerationMode,
        writer: Arc<super::journal::WriterLock>,
    ) -> Result<Self> {
        let path = path(journal, epoch);
        safe_path(&path)?;
        if path.exists() {
            return Err(Error::ArchiveCorrupt);
        }
        let state = GenerationAudit {
            version: 1,
            epoch,
            fingerprint,
            mode,
            calls: vec![],
        };
        write(&path, &state)?;
        Ok(Self {
            path,
            _writer: writer,
            state: Mutex::new((state, false)),
        })
    }
    fn mutate<T>(&self, change: impl FnOnce(&mut GenerationAudit) -> Result<T>) -> Result<T> {
        let mut state = self.state.lock().map_err(|_| Error::HostFailed)?;
        if state.1 {
            return Err(Error::HostFailed);
        }
        let mut next = state.0.clone();
        let value = change(&mut next)?;
        if let Err(error) = write(&self.path, &next) {
            state.1 = true;
            return Err(error);
        }
        state.0 = next;
        Ok(value)
    }
    pub(super) fn begin(&self) -> Result<u64> {
        self.mutate(|audit| {
            let id = u64::try_from(audit.calls.len())
                .map_err(|_| Error::HostFailed)?
                .checked_add(1)
                .ok_or(Error::HostFailed)?;
            audit.calls.push(GenerationCallReceipt {
                id,
                status: GenerationCallStatus::Pending,
                items: vec![],
            });
            Ok(id)
        })
    }
    pub(super) fn finish(
        &self,
        id: u64,
        status: GenerationCallStatus,
        items: Vec<GenerationDisposition>,
    ) -> Result<()> {
        self.mutate(|audit| {
            let call = audit
                .calls
                .iter_mut()
                .find(|call| call.id == id)
                .ok_or(Error::HostFailed)?;
            if call.status != GenerationCallStatus::Pending
                || status == GenerationCallStatus::Pending
                || (status != GenerationCallStatus::Returned && !items.is_empty())
            {
                return Err(Error::HostFailed);
            }
            call.status = status;
            call.items = items;
            Ok(())
        })
    }
    pub(super) fn settled(&self) -> Result<()> {
        let state = self.state.lock().map_err(|_| Error::HostFailed)?;
        if state.1
            || state
                .0
                .calls
                .iter()
                .any(|c| c.status == GenerationCallStatus::Pending)
        {
            return Err(Error::HostFailed);
        }
        Ok(())
    }
}
fn write(path: &Path, audit: &GenerationAudit) -> Result<()> {
    safe_path(path)?;
    let bytes = serde_json::to_vec(audit).map_err(|_| Error::ArchiveIo)?;
    if u64::try_from(bytes.len()).map_err(|_| Error::ArchiveIo)? > MAX_BYTES {
        return Err(Error::ArchiveIo);
    }
    let parent = path.parent().ok_or(Error::ArchiveIo)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|_| Error::ArchiveIo)?;
    temp.write_all(&bytes).map_err(|_| Error::ArchiveIo)?;
    temp.as_file().sync_all().map_err(|_| Error::ArchiveIo)?;
    temp.persist(path).map_err(|_| Error::ArchiveIo)?;
    File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::ArchiveIo)
}
