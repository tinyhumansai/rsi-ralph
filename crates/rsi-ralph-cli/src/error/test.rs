//! Error rendering: the cause chain must survive into what users see.
use super::*;

#[derive(Debug, thiserror::Error)]
#[error("cannot build the agent for seat {seat}")]
struct Outer {
    seat: String,
    #[source]
    source: Inner,
}

#[derive(Debug, thiserror::Error)]
#[error("unknown tool `read_dif`")]
struct Inner;

#[derive(Debug, thiserror::Error)]
#[error("proposer: {0}")]
struct Repeats(#[source] Inner);

#[test]
fn chain_appends_every_hidden_cause() {
    let error = Outer {
        seat: "orchestrator".into(),
        source: Inner,
    };
    assert_eq!(
        chain(&error),
        "cannot build the agent for seat orchestrator: unknown tool `read_dif`"
    );
}

#[test]
fn chain_does_not_repeat_a_cause_already_in_the_message() {
    assert_eq!(chain(&Repeats(Inner)), "proposer: unknown tool `read_dif`");
}

#[test]
fn chain_of_a_plain_error_is_its_message() {
    assert_eq!(chain(&Inner), "unknown tool `read_dif`");
}
