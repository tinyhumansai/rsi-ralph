//! The single-owner lock on the total ledger.
//!
//! Two gateways charging one total ledger would each believe they own the
//! remaining budget. [`LedgerLock`] creates `<ledger>.lock` exclusively and
//! removes it on drop; a second gateway refuses to start while it exists.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// An exclusive claim on a ledger, released on drop.
#[derive(Debug)]
pub(crate) struct LedgerLock {
    path: PathBuf,
}

impl LedgerLock {
    /// Claims `ledger` by creating `<ledger>.lock` with this process's id.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Locked`] if the lock file exists, or [`Error::Io`].
    pub(crate) fn acquire(ledger: &Path) -> Result<Self> {
        let path = ledger.with_extension("lock");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::AlreadyExists {
                    Error::Locked(path.clone())
                } else {
                    Error::Io {
                        path: path.clone(),
                        source,
                    }
                }
            })?;
        // The pid is a courtesy for whoever finds a stale lock; failing to
        // write it does not weaken the exclusive create above.
        let _ = writeln!(file, "{}", std::process::id());
        Ok(Self { path })
    }
}

impl Drop for LedgerLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod test;
