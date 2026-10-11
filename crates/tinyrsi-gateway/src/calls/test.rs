//! Unit tests for the upstream call tally and refusal messages.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{CallMark, CallTally, MESSAGE_CHARS, Refusal, UpstreamCalls, refusal_message};

#[test]
fn counts_answers_and_keeps_the_latest_refusal() {
    let tally = CallTally::default();
    let calls: Vec<_> = (0..3).map(|_| tally.dispatched()).collect();
    tally.answered(calls[0], 200, b"{}");
    tally.answered(calls[1], 429, br#"{"error":{"message":"Rate limited","code":429}}"#);
    tally.answered(calls[2], 403, br#"{"error":{"message":"Key limit exceeded","code":403}}"#);
    assert_eq!(
        tally.since(CallMark::default()),
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
    let call = tally.dispatched();
    tally.unreachable(call, "error sending request\n  connection refused");
    let calls = tally.since(CallMark::default());
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
    let calls = tally.since(CallMark::default());
    assert_eq!((calls.sent, calls.refused), (1, 0));
    assert!(!calls.all_refused());
}

#[test]
fn a_late_refusal_stays_with_the_calls_sent_before_the_mark() {
    let tally = CallTally::default();
    let start = tally.mark();
    let early = tally.dispatched();
    let mark = tally.mark();
    assert_eq!(mark, CallMark(1));
    let late = tally.dispatched();
    tally.answered(late, 200, b"{}");
    // The call sent before the mark is refused only now.
    tally.answered(early, 403, br#"{"error":{"message":"no"}}"#);
    assert_eq!(
        tally.since(mark),
        UpstreamCalls {
            sent: 1,
            refused: 0,
            last_refusal: None
        }
    );
    assert!(!tally.since(mark).all_refused());
    let all = tally.since(start);
    assert_eq!((all.sent, all.refused), (2, 1));
    assert_eq!(all.last_refusal.unwrap().status, 403);
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
fn all_refused_needs_a_call() {
    assert!(!UpstreamCalls::default().all_refused());
    let refused = UpstreamCalls {
        sent: 2,
        refused: 2,
        last_refusal: None,
    };
    assert!(refused.all_refused());
}
