//! Unit tests for the scoring error type.

use super::*;

#[test]
fn keeps_the_messages_callers_already_see() {
    assert_eq!(
        Error::InvalidEvaluation.to_string(),
        "invalid evaluation data or policy"
    );
    assert_eq!(Error::HostFailed.to_string(), "host operation failed");
    assert_eq!(
        Error::JudgeMalformed.to_string(),
        "judge reply is not a well-formed grade"
    );
    assert_eq!(
        Error::from(rsi_ralph_model::Error::CallFailed).to_string(),
        "model call failed"
    );
}

#[test]
fn a_candidate_failure_names_the_candidate_not_the_host() {
    let failure = rsi_ralph_types::CandidateFailure {
        kind: rsi_ralph_types::CandidateFailureKind::Crash,
        failed_runs: 1,
        runs: 2,
    };
    let error = Error::CandidateFailed(failure);
    assert_eq!(
        error.to_string(),
        "candidate failed runs the incumbent completed"
    );
    assert_ne!(error, Error::HostFailed);
}
