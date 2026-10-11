//! The messages shared with the core crate are stored in ledgers; pin them.

use super::*;

#[test]
fn keeps_the_messages_ledgers_already_store() {
    for (error, message) in [
        (
            Error::InvalidProposal,
            "invalid proposal or epoch configuration",
        ),
        (Error::HostFailed, "host operation failed"),
        (Error::ArchiveIo, "archive storage operation failed"),
        (Error::ArchiveCorrupt, "archive is corrupt"),
        (Error::ArchiveLocked, "archive is already locked"),
        (
            Error::InvalidScriptSubject,
            "invalid script subject or loop policy",
        ),
        (
            Error::SupervisorMalformed,
            "supervisor reply is not a well-formed proposal",
        ),
        (
            Error::ProposalPathOutsideEditable,
            "proposal edits a path outside the editable set",
        ),
        (Error::Exhausted, "scripted evolver has no proposals left"),
        (
            Error::Model(tinyrsi_model::Error::CallFailed),
            "model call failed",
        ),
    ] {
        assert_eq!(error.to_string(), message);
    }
}
