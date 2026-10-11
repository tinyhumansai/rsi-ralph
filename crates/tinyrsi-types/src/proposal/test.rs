//! Serde pins for proposal data. These are stored forms: archives and
//! retreat outputs written by one version must decode in the next.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use serde_json::json;
use tinyhivemind_lang::patch::{Operation, Patch, PatchClass};

#[test]
fn every_reason_code_serializes_in_screaming_snake_case() {
    let codes = [
        (ReasonCode::Untestable, "UNTESTABLE"),
        (ReasonCode::Ungrounded, "UNGROUNDED"),
        (ReasonCode::WrongClass, "WRONG_CLASS"),
        (ReasonCode::Guard, "GUARD"),
        (ReasonCode::Unformalizable, "UNFORMALIZABLE"),
        (ReasonCode::Duplicate, "DUPLICATE"),
        (ReasonCode::NoGain, "NO_GAIN"),
        (ReasonCode::Inconclusive, "INCONCLUSIVE"),
        (ReasonCode::NotReplicated, "NOT_REPLICATED"),
        (ReasonCode::ApprovalDenied, "APPROVAL_DENIED"),
        (ReasonCode::HostFailure, "HOST_FAILURE"),
        (ReasonCode::Accepted, "ACCEPTED"),
    ];
    for (code, text) in codes {
        assert_eq!(serde_json::to_value(code).unwrap(), json!(text));
        assert_eq!(
            serde_json::from_value::<ReasonCode>(json!(text)).unwrap(),
            code
        );
    }
}

#[test]
fn a_prediction_rejects_unknown_fields() {
    let decoded = serde_json::from_value::<Prediction>(json!({
        "metric": "normalized_h",
        "minimum_gain": 0.02,
        "nodes": [],
        "confidence": 0.9,
    }));
    assert!(decoded.is_err());
}

#[test]
fn a_prediction_serializes_to_its_stored_form() {
    let prediction = Prediction {
        metric: "normalized_h".into(),
        minimum_gain: 0.02,
        nodes: vec!["parse".into()],
    };
    assert_eq!(
        serde_json::to_value(&prediction).unwrap(),
        json!({"metric": "normalized_h", "minimum_gain": 0.02, "nodes": ["parse"]})
    );
}

fn patch() -> Patch {
    Patch {
        class: PatchClass::Context,
        operations: vec![Operation::Context {
            path: "context/main.md".into(),
            item: "method".into(),
            before: Some("Guess.\n".into()),
            after: Some("Compute and check.\n".into()),
        }],
    }
}

fn proposal() -> Proposal {
    Proposal {
        title: "Verify".into(),
        patch: patch(),
        mechanism: "Replace guessing with execution".into(),
        prediction: Prediction {
            metric: "verified_quality".into(),
            minimum_gain: 0.05,
            nodes: vec!["n1".into()],
        },
        transcript_refs: vec![1],
        risk: "extra tokens".into(),
        effort: "one delta".into(),
    }
}

/// Serializes `value`, checks it round-trips, then adds one unknown field and
/// returns whether that still decodes.
fn accepts_an_extra_field<T>(value: &T) -> bool
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let mut encoded = serde_json::to_value(value).unwrap();
    assert_eq!(
        &serde_json::from_value::<T>(encoded.clone()).unwrap(),
        value
    );
    encoded
        .as_object_mut()
        .unwrap()
        .insert("extra".into(), json!(true));
    serde_json::from_value::<T>(encoded).is_ok()
}

#[test]
fn a_proposal_round_trips_and_rejects_unknown_fields() {
    assert!(!accepts_an_extra_field(&proposal()));
}

#[test]
fn a_proposal_event_round_trips_and_rejects_unknown_fields() {
    let event = ProposalEvent {
        epoch: 1,
        patch: patch(),
        rationale: "r".into(),
        reason: "GUARD".into(),
        transcript_refs: vec![2],
    };
    assert!(!accepts_an_extra_field(&event));
}
