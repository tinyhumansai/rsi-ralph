//! Unit tests for the crate-wide error type.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

#[test]
fn renders_a_human_readable_message_for_an_empty_field() {
    assert_eq!(
        Error::EmptyField.to_string(),
        "lineage entry has an empty snapshot address or rationale"
    );
}

#[test]
fn renders_a_human_readable_message_for_an_out_of_order_step() {
    assert_eq!(
        Error::StepOutOfOrder.to_string(),
        "lineage entry's step does not continue the ledger"
    );
}

#[test]
fn is_a_standard_error() {
    fn assert_error<E: std::error::Error>(_: &E) {}

    assert_error(&Error::EmptyField);
    assert_error(&Error::StepOutOfOrder);
}

#[test]
fn renders_lowercase_messages_for_script_loop_failures() {
    for (error, message) in [
        (Error::TraceIo, "loop trace write failed"),
        (Error::InvalidScriptSubject, "invalid script subject or loop policy"),
        (Error::ScriptBaselineFailed, "baseline script failed"),
        (Error::ScriptTimeout, "script run timed out"),
        (Error::ScriptOutputExceeded, "script run exceeded its output cap"),
        (Error::ScriptFailed, "script run exited unsuccessfully"),
        (Error::MetricMissing, "script run printed no metric line"),
        (
            Error::MetricInvalid,
            "script run printed a non-finite or unparsable metric",
        ),
        (
            Error::ProposalPathOutsideEditable,
            "proposal edits a path outside the editable set",
        ),
        (
            Error::SupervisorMalformed,
            "supervisor reply is not a well-formed proposal",
        ),
    ] {
        assert_eq!(error.to_string(), message);
    }
}
