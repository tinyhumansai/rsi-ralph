//! The per-call meter: one JSON line per call admitted or refused.
//!
//! Rows carry the caller's tags (`phase`, `seat`, `candidate`), so a run's
//! spend can be broken down by loop phase or seat afterwards.

mod types;

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub(crate) use types::{MeterRow, Tags};

/// An append-only JSONL file.
#[derive(Debug)]
pub(crate) struct Meter {
    path: PathBuf,
}

impl Meter {
    /// Uses `path`, creating its directory.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the directory cannot be created.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Appends one row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be written.
    pub(crate) fn record(&self, row: &MeterRow) -> Result<()> {
        let io = |source| Error::Io {
            path: self.path.clone(),
            source,
        };
        let mut line = serde_json::to_vec(row).map_err(|error| io(std::io::Error::other(error)))?;
        line.push(b'\n');
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut file| file.write_all(&line))
            .map_err(io)
    }
}

#[cfg(test)]
mod test;
