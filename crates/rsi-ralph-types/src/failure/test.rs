//! Serde pins for candidate failures. These are stored forms: archive
//! evidence written by one version must decode in the next.

#![expect(clippy::unwrap_used, reason = "assert deterministic serde fixtures")]

use super::*;
use serde_json::json;

#[test]
fn every_failure_kind_serializes_in_snake_case() {
    for (kind, text) in [
        (CandidateFailureKind::Crash, "crash"),
        (CandidateFailureKind::Timeout, "timeout"),
        (CandidateFailureKind::OverBudget, "over_budget"),
        (CandidateFailureKind::Unsupported, "unsupported"),
    ] {
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(text));
        assert_eq!(
            serde_json::from_value::<CandidateFailureKind>(json!(text)).unwrap(),
            kind
        );
    }
}

#[test]
fn a_candidate_failure_serializes_to_its_stored_form() {
    let failure = CandidateFailure {
        kind: CandidateFailureKind::Timeout,
        failed_runs: 2,
        runs: 4,
    };
    let wire = json!({"kind": "timeout", "failed_runs": 2, "runs": 4});
    assert_eq!(serde_json::to_value(failure).unwrap(), wire);
    assert_eq!(
        serde_json::from_value::<CandidateFailure>(wire).unwrap(),
        failure
    );
}

#[test]
fn a_candidate_failure_rejects_unknown_fields() {
    let decoded = serde_json::from_value::<CandidateFailure>(json!({
        "kind": "crash", "failed_runs": 1, "runs": 1, "stderr": "private",
    }));
    assert!(decoded.is_err());
}
