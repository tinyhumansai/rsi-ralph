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
        (
            Error::InvalidScriptSubject,
            "invalid script subject or loop policy",
        ),
        (Error::ScriptBaselineFailed, "baseline script failed"),
        (Error::ScriptTimeout, "script run timed out"),
        (
            Error::ScriptOutputExceeded,
            "script run exceeded its output cap",
        ),
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

#[test]
fn model_errors_map_onto_the_variants_callers_already_match() {
    assert_eq!(
        Error::from(tinyrsi_model::Error::InvalidRequest),
        Error::InvalidEvaluation
    );
    assert_eq!(
        Error::from(tinyrsi_model::Error::CallFailed),
        Error::HostFailed
    );
}

#[test]
fn scoring_errors_map_onto_the_variants_callers_already_match() {
    use tinyrsi_score::Error as Score;
    assert_eq!(
        Error::from(Score::InvalidEvaluation),
        Error::InvalidEvaluation
    );
    assert_eq!(Error::from(Score::HostFailed), Error::HostFailed);
    assert_eq!(Error::from(Score::JudgeMalformed), Error::JudgeMalformed);
    let failure = crate::CandidateFailure {
        kind: crate::CandidateFailureKind::Timeout,
        failed_runs: 2,
        runs: 4,
    };
    assert_eq!(
        Error::from(Score::CandidateFailed(failure)),
        Error::CandidateFailed(failure)
    );
    assert_eq!(
        Error::CandidateFailed(failure).to_string(),
        "candidate failed runs the incumbent completed"
    );
    assert_eq!(
        Error::from(Score::Model(tinyrsi_model::Error::CallFailed)),
        Error::HostFailed
    );
    assert_eq!(
        Error::JudgeMalformed.to_string(),
        "judge reply is not a well-formed grade"
    );
}

#[test]
fn checker_errors_map_onto_the_variants_callers_already_match() {
    assert_eq!(
        Error::from(tinyrsi_verify::Error::InvalidBlueprint),
        Error::InvalidBlueprint
    );
    assert_eq!(
        Error::from(tinyrsi_verify::Error::HostFailed),
        Error::HostFailed
    );
}

#[test]
fn evolution_errors_keep_the_messages_ledgers_store() {
    use tinyrsi_evolve::Error as Evolve;
    for (from, to) in [
        (Evolve::InvalidProposal, Error::InvalidProposal),
        (Evolve::ArchiveIo, Error::ArchiveIo),
        (Evolve::ArchiveCorrupt, Error::ArchiveCorrupt),
        (Evolve::ArchiveLocked, Error::ArchiveLocked),
        (Evolve::InvalidScriptSubject, Error::InvalidScriptSubject),
        (Evolve::SupervisorMalformed, Error::SupervisorMalformed),
        (
            Evolve::ProposalPathOutsideEditable,
            Error::ProposalPathOutsideEditable,
        ),
        (Evolve::HostFailed, Error::HostFailed),
        (Evolve::Exhausted, Error::HostFailed),
        (
            Evolve::Model(tinyrsi_model::Error::InvalidRequest),
            Error::InvalidEvaluation,
        ),
    ] {
        let message = from.to_string();
        let mapped = Error::from(from);
        if mapped != Error::HostFailed && mapped != Error::InvalidEvaluation {
            assert_eq!(mapped.to_string(), message);
        }
        assert_eq!(mapped, to);
    }
}
