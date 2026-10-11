//! Subjects and their checkouts.

use std::path::PathBuf;

/// Where the incumbent lives and how candidates are made from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// A plain folder; the incumbent is a copy inside the run folder.
    Dir {
        /// The loop's subject folder, never modified.
        source: PathBuf,
    },
    /// A git repository; the incumbent is the tip of `branch`.
    Git {
        /// The repository.
        repo: PathBuf,
        /// Branch holding the incumbent; promotion fast-forwards it.
        branch: String,
    },
}

/// One materialized checkout: the incumbent or a candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout {
    /// `incumbent` or the candidate id.
    pub label: String,
    /// The folder.
    pub path: PathBuf,
}
