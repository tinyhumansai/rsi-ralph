//! Crate-wide error and result types.
//!
//! Variants shared with the `tinyrsi` crate keep their names and messages.
//! The script loop writes a rejected proposal's message into its ledger, so
//! those strings are part of the stored form.

/// Errors returned by this crate.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A proposal, retreat request or response is incomplete or unsafe.
    #[error("invalid proposal or epoch configuration")]
    InvalidProposal,
    /// A host-owned transport or engine operation failed.
    #[error("host operation failed")]
    HostFailed,
    /// Retreat memory storage could not complete its operation.
    #[error("archive storage operation failed")]
    ArchiveIo,
    /// Stored retreat memory cannot be read back safely.
    #[error("archive is corrupt")]
    ArchiveCorrupt,
    /// Another writer holds the retreat memory.
    #[error("archive is already locked")]
    ArchiveLocked,
    /// Supervisor limits are unusable.
    #[error("invalid script subject or loop policy")]
    InvalidScriptSubject,
    /// The supervisor's reply is not exactly one well-formed proposal.
    #[error("supervisor reply is not a well-formed proposal")]
    SupervisorMalformed,
    /// A proposal tried to write a path outside the editable set.
    #[error("proposal edits a path outside the editable set")]
    ProposalPathOutsideEditable,
    /// A scripted evolver ran out of canned proposals.
    #[error("scripted evolver has no proposals left")]
    Exhausted,
    /// A model call failed, overspent or was refused.
    #[error(transparent)]
    Model(#[from] rsi_ralph_model::Error),
}

/// The crate's standard result type.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
