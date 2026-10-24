//! The single-owner lock on the total ledger.
//!
//! Two gateways charging one total ledger would each believe they own the
//! remaining budget. [`LedgerLock`] creates `<ledger>.lock` exclusively and
//! removes it on drop; a second gateway refuses to start while it exists.
//! A process killed by a signal never drops its lock, so a lock whose
//! recorded pid no longer runs is stale and is taken over.

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
        let mut file = match create(&path) {
            Err(Error::Locked(_)) if stale(&path) => {
                let _ = std::fs::remove_file(&path);
                create(&path)?
            }
            other => other?,
        };
        // The pid is a courtesy for whoever finds a stale lock; failing to
        // write it does not weaken the exclusive create above.
        let _ = writeln!(file, "{}", std::process::id());
        Ok(Self { path })
    }
}

fn create(path: &Path) -> Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::AlreadyExists {
                Error::Locked(path.to_path_buf())
            } else {
                Error::Io {
                    path: path.to_path_buf(),
                    source,
                }
            }
        })
}

/// Whether the lock at `path` names a pid that is no longer running. An
/// unreadable lock, or one whose owner cannot be checked, is not stale.
fn stale(path: &Path) -> bool {
    let Some(pid) = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
    else {
        return false;
    };
    if pid == std::process::id() {
        return false;
    }
    // `ps -p` exits non-zero exactly when no such process exists, whoever
    // owns it; `kill -0` would also fail for another user's live process.
    std::process::Command::new("ps")
        .args(["-p", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.code() == Some(1))
}

impl Drop for LedgerLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod test;
