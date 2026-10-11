//! Evolving a plain script: the supervisor and the proposals it makes.
//!
//! The script loop in `tinyrsi` renders a [`ScriptBrief`] from its ledger and
//! asks a [`ScriptSupervisor`] for a [`ScriptProposal`]: whole-file
//! replacements of editable files, with a rationale and a checkable
//! hypothesis. Applying the proposal, running the candidate and scoring it
//! stay with the loop.
mod supervisor;
mod types;

use crate::{Error, Result};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
pub use supervisor::ScriptSupervisor;
pub use types::{ProposedFile, ScriptBrief, ScriptProposal, SupervisorLimits};

/// Longest approach label, in characters, that
/// [`ScriptProposal::approach_label`] keeps.
pub(crate) const APPROACH_CHARS: usize = 60;

/// True when `path` is relative and made only of normal components.
#[must_use]
pub fn is_plain_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

impl ScriptProposal {
    /// The approach label with whitespace runs collapsed to one space and cut
    /// to 60 characters (the last one an ellipsis), or `None` when blank.
    ///
    /// Over-long labels are cut rather than refused: the proposal cost a full
    /// model call and its code may still be good.
    #[must_use]
    pub fn approach_label(&self) -> Option<String> {
        let label = self.approach.split_whitespace().collect::<Vec<_>>().join(" ");
        if label.is_empty() {
            return None;
        }
        if label.chars().count() <= APPROACH_CHARS {
            return Some(label);
        }
        let mut cut: String = label.chars().take(APPROACH_CHARS - 1).collect();
        cut.push('…');
        Some(cut)
    }

    /// Checks the proposal against the subject's editable set.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SupervisorMalformed`] for an empty rationale,
    /// hypothesis or file list, or a path written twice, and
    /// [`Error::ProposalPathOutsideEditable`] for an absolute path, a path with
    /// `.` or `..` components, or a path not in `editable`.
    pub fn validate(&self, editable: &[PathBuf]) -> Result<()> {
        if self.rationale.trim().is_empty()
            || self.hypothesis.trim().is_empty()
            || self.files.is_empty()
        {
            return Err(Error::SupervisorMalformed);
        }
        let mut seen = BTreeSet::new();
        for file in &self.files {
            let path = Path::new(&file.path);
            if !is_plain_relative(path) || !editable.iter().any(|e| e == path) {
                return Err(Error::ProposalPathOutsideEditable);
            }
            if !seen.insert(path) {
                return Err(Error::SupervisorMalformed);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test;
