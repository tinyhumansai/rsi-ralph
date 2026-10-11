//! Unit tests for the upstream call tally and refusal messages.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{CallTally, MESSAGE_CHARS, Refusal, UpstreamCalls, refusal_message};

#[test]
fn counts_answers_and_keeps_the_latest_refusal() {
    let tally = CallTally::default();
    for _ in 0..3 {
        tally.dispatched();
    }
    tally.answered(200, b"{}");
    tally.answered(429, br#"{"error":{"message":"Rate limited","code":429}}"#);
    tally.answered(
        403,
        br#"{"error":{"message":"Key limit exceeded","code":403}}"#,
    );
    assert_eq!(
        tally.snapshot(),
        UpstreamCalls {
            sent: 3,
            refused: 2,
            last_refusal: Some(Refusal {
                status: 403,
                message: "Key limit exceeded".into()
            }),
        }
    );
}

#[test]
fn an_unreachable_provider_is_a_502_refusal() {
    let tally = CallTally::default();
    tally.dispatched();
    tally.unreachable("error sending request\n  connection refused");
    let calls = tally.snapshot();
    assert!(calls.all_refused());
    assert_eq!(
        calls.last_refusal,
        Some(Refusal {
            status: 502,
            message: "error sending request connection refused".into()
        })
    );
}

#[test]
fn a_call_still_in_flight_is_already_sent() {
    let tally = CallTally::default();
    tally.dispatched();
    let calls = tally.snapshot();
    assert_eq!((calls.sent, calls.refused), (1, 0));
    assert!(!calls.all_refused());
}

#[test]
fn reads_the_message_from_each_error_shape() {
    assert_eq!(refusal_message(br#"{"error":"bad"}"#), "bad");
    assert_eq!(refusal_message(b"oops\n"), "oops");
    assert_eq!(refusal_message(br#"{"detail":"x"}"#), r#"{"detail":"x"}"#);
    let long = format!(r#"{{"error":{{"message":"{}"}}}}"#, "a".repeat(400));
    let message = refusal_message(long.as_bytes());
    assert_eq!(message.chars().count(), MESSAGE_CHARS);
    assert!(message.ends_with('…'));
}

#[test]
fn since_counts_only_later_calls_and_their_refusals() {
    let refusal = Refusal {
        status: 403,
        message: "no".into(),
    };
    let earlier = UpstreamCalls {
        sent: 4,
        refused: 1,
        last_refusal: Some(refusal.clone()),
    };
    let quiet = UpstreamCalls {
        sent: 6,
        ..earlier.clone()
    };
    assert_eq!(
        quiet.since(&earlier),
        UpstreamCalls {
            sent: 2,
            refused: 0,
            last_refusal: None
        }
    );
    assert!(!quiet.since(&earlier).all_refused());
    let refused = UpstreamCalls {
        sent: 6,
        refused: 3,
        last_refusal: Some(refusal.clone()),
    };
    let delta = refused.since(&earlier);
    assert_eq!((delta.sent, delta.refused), (2, 2));
    assert!(delta.all_refused());
    assert_eq!(delta.last_refusal, Some(refusal));
    assert!(!UpstreamCalls::default().all_refused());
}
