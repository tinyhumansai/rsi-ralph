//! Unit tests for the `Append` payloads.
//!
//! These pin the serde representation. It is the wire form: a host and a module
//! that disagree about a field name fail at runtime with a decode error, so the
//! shape is asserted here rather than assumed.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{
    GradeEntry, LineageAppendRequest, LineageAppendResponse, LineageHeadResponse, Verdict,
};

#[test]
fn an_append_request_serializes_to_its_wire_form() {
    let entry = GradeEntry::new(0, "sha256:abc", 0.703, Verdict::Accepted, "baseline");
    let encoded = serde_json::to_value(LineageAppendRequest::new(entry)).unwrap();
    assert_eq!(
        encoded,
        serde_json::json!({
            "entry": {
                "step": 0,
                "snapshot": "sha256:abc",
                "grade": 0.703,
                "verdict": "Accepted",
                "rationale": "baseline",
            }
        })
    );
}

#[test]
fn an_append_response_serializes_to_its_wire_form() {
    let encoded = serde_json::to_value(LineageAppendResponse::new(true)).unwrap();
    assert_eq!(encoded, serde_json::json!({ "recorded": true }));
}

#[test]
fn an_append_request_round_trips_through_json() {
    let request = LineageAppendRequest::new(GradeEntry::new(
        3,
        "sha256:def",
        0.778,
        Verdict::Rejected,
        "within noise",
    ));
    let encoded = serde_json::to_string(&request).unwrap();
    assert_eq!(
        serde_json::from_str::<LineageAppendRequest>(&encoded).unwrap(),
        request
    );
}

#[test]
fn a_head_response_round_trips_through_json() {
    let response = LineageHeadResponse::new(Some(GradeEntry::new(
        0,
        "sha256:abc",
        0.5,
        Verdict::Accepted,
        "baseline",
    )));
    let encoded = serde_json::to_string(&response).unwrap();
    assert_eq!(
        serde_json::from_str::<LineageHeadResponse>(&encoded).unwrap(),
        response
    );
}

#[test]
fn an_empty_head_response_round_trips_through_json() {
    let response = LineageHeadResponse::new(None);
    let encoded = serde_json::to_string(&response).unwrap();
    assert_eq!(
        serde_json::from_str::<LineageHeadResponse>(&encoded).unwrap(),
        response
    );
}

#[test]
fn an_append_request_missing_its_entry_is_rejected() {
    let decoded = serde_json::from_value::<LineageAppendRequest>(serde_json::json!({}));
    assert!(decoded.is_err());
}

#[test]
fn an_append_request_missing_its_verdict_is_rejected() {
    let decoded = serde_json::from_value::<LineageAppendRequest>(serde_json::json!({
        "entry": {
            "step": 0,
            "snapshot": "sha256:abc",
            "grade": 0.5,
            "rationale": "baseline",
        }
    }));
    assert!(decoded.is_err());
}

#[test]
fn an_unknown_verdict_is_rejected() {
    let decoded = serde_json::from_value::<Verdict>(serde_json::json!("Pending"));
    assert!(decoded.is_err());
}

#[test]
fn constructors_accept_both_borrowed_and_owned_strings() {
    assert_eq!(
        GradeEntry::new(
            0,
            String::from("sha256:abc"),
            0.5,
            Verdict::Accepted,
            String::from("r")
        ),
        GradeEntry::new(0, "sha256:abc", 0.5, Verdict::Accepted, "r")
    );
}
