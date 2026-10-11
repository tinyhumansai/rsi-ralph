//! Crate-wide error and result types.
//!
//! Every fallible public function in this crate returns [`Result`], and every
//! failure mode is a distinct [`Error`] variant. Add a variant rather than
//! encoding new context into an existing message: callers match on variants,
//! and message text is not a stable API.
//!
//! Variants carry the data a caller needs to react, keep their `#[error]`
//! message lowercase and free of trailing punctuation, and are documented so
//! the rendered rustdoc explains when each one occurs.

/// Errors returned by this crate.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A lineage entry named a snapshot address or rationale that was empty
    /// or only whitespace.
    #[error("lineage entry has an empty snapshot address or rationale")]
    EmptyField,
    /// A lineage entry's step did not equal the ledger's current length.
    #[error("lineage entry's step does not continue the ledger")]
    StepOutOfOrder,
    /// Paired evaluation data, budgets, or statistical policy are invalid.
    #[error("invalid evaluation data or policy")]
    InvalidEvaluation,
    /// Archive storage could not complete its operation.
    #[error("archive storage operation failed")]
    ArchiveIo,
    /// A stored archive cannot be replayed safely.
    #[error("archive is corrupt")]
    ArchiveCorrupt,
    /// Another writer holds this archive.
    #[error("archive is already locked")]
    ArchiveLocked,
    /// A candidate's identity, patch or ancestry is invalid.
    #[error("invalid candidate record")]
    InvalidRecord,
    /// The active design has no accepted parent to restore.
    #[error("active candidate has no accepted parent")]
    NoParent,
    /// A blueprint has invalid nodes, dependencies or verification order.
    #[error("invalid blueprint or verification outcome")]
    InvalidBlueprint,
    /// The coordinator could not provide a complete observation.
    #[error("coordinator observation failed")]
    ObservationFailed,
    /// A proposal or host configuration is incomplete.
    #[error("invalid proposal or epoch configuration")]
    InvalidProposal,
    /// A host-owned evaluation or approval operation failed.
    #[error("host operation failed")]
    HostFailed,
    /// Turn callbacks or epoch identities conflict with retained host evidence.
    #[error("invalid or conflicting host evidence")]
    InvalidEvidence,
    /// Unfinished journaled turns prevent an epoch from consuming their evidence.
    #[error("host evidence has unfinished turns")]
    EvidencePending,
}

/// The crate's standard result type.
///
/// Use this alias in public signatures instead of spelling out
/// `std::result::Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
