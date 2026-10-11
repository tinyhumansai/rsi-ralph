//! Exclusive atomic storage for redacted evidence snapshots.
use super::{State, validate};
use crate::{Error, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(super) struct Store {
    pub state: State,
    path: PathBuf,
    lock: File,
}
impl Store {
    pub(super) fn open(input: &Path) -> Result<Self> {
        let parent = input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|_| Error::ArchiveIo)?;
        let path = fs::canonicalize(parent)
            .map_err(|_| Error::ArchiveIo)?
            .join(input.file_name().ok_or(Error::ArchiveIo)?);
        let lockpath = sidecar(&path, "lock");
        if [&path, &lockpath]
            .iter()
            .any(|p| fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
        {
            return Err(Error::ArchiveIo);
        }
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(lockpath)
            .map_err(|_| Error::ArchiveIo)?;
        fs2::FileExt::try_lock_exclusive(&lock).map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                Error::ArchiveLocked
            } else {
                Error::ArchiveIo
            }
        })?;
        match fs::remove_file(sidecar(&path, "next")) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::ArchiveIo),
        }
        let state = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| Error::ArchiveCorrupt)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(_) => return Err(Error::ArchiveIo),
        };
        validate(&state)?;
        Ok(Self { state, path, lock })
    }
    pub(super) fn save(&mut self, next: State) -> Result<()> {
        validate(&next)?;
        let bytes = serde_json::to_vec(&next).map_err(|_| Error::ArchiveIo)?;
        let temporary = sidecar(&self.path, "next");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| Error::ArchiveIo)?;
        let result = (|| {
            file.write_all(&bytes).map_err(|_| Error::ArchiveIo)?;
            file.sync_all().map_err(|_| Error::ArchiveIo)?;
            fs::rename(&temporary, &self.path).map_err(|_| Error::ArchiveIo)?;
            #[cfg(unix)]
            File::open(self.path.parent().ok_or(Error::ArchiveIo)?)
                .and_then(|f| f.sync_all())
                .map_err(|_| Error::ArchiveIo)?;
            Ok(())
        })();
        let _ = fs::remove_file(temporary);
        if result.is_ok() || fs::read(&self.path).is_ok_and(|disk| disk == bytes) {
            self.state = next;
        }
        result
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.lock);
    }
}
pub(super) fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".{suffix}"));
    PathBuf::from(name)
}
