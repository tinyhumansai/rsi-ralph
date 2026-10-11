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
    /// A loop trace sink could not record an event.
    #[error("loop trace write failed")]
    TraceIo,
    /// A script subject or loop policy is incomplete or unsafe.
    #[error("invalid script subject or loop policy")]
    InvalidScriptSubject,
    /// The starting script failed on at least one seed.
    #[error("baseline script failed")]
    ScriptBaselineFailed,
    /// A script run outlived its timeout and was killed.
    #[error("script run timed out")]
    ScriptTimeout,
    /// A script run wrote more output than its cap and was killed.
    #[error("script run exceeded its output cap")]
    ScriptOutputExceeded,
    /// A script run exited with a non-zero status or was killed by a signal.
    #[error("script run exited unsuccessfully")]
    ScriptFailed,
    /// A script run printed no `METRIC <name>=<value>` line for the metric.
    #[error("script run printed no metric line")]
    MetricMissing,
    /// A script run printed a metric that is not a finite number.
    #[error("script run printed a non-finite or unparsable metric")]
    MetricInvalid,
    /// A proposal tried to write a path outside the editable set.
    #[error("proposal edits a path outside the editable set")]
    ProposalPathOutsideEditable,
    /// The supervisor's reply is not exactly one well-formed proposal.
    #[error("supervisor reply is not a well-formed proposal")]
    SupervisorMalformed,
}

/// The crate's standard result type.
///
/// Use this alias in public signatures instead of spelling out
/// `std::result::Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
