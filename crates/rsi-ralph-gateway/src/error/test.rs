//! Unit tests for the gateway error type.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::Error;

#[test]
fn budget_errors_name_the_ledger_and_amounts() {
    let error = Error::BudgetExceeded {
        ledger: "run",
        needed: 0.5,
        remaining: 0.25,
    };
    assert_eq!(
        error.to_string(),
        "run budget exhausted: call may cost $0.500000, $0.250000 remains"
    );
}

#[test]
fn io_errors_keep_their_source() {
    let error = Error::Io {
        path: "ledger.json".into(),
        source: std::io::Error::other("denied"),
    };
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn runtime_and_serve_errors_keep_their_source() {
    let runtime = Error::Runtime(std::io::Error::other("no threads"));
    assert_eq!(
        runtime.to_string(),
        "cannot start the gateway runtime: no threads"
    );
    assert!(std::error::Error::source(&runtime).is_some());
    let serve = Error::Serve(std::io::Error::other("accept"));
    assert_eq!(serve.to_string(), "gateway server failed: accept");
    assert!(std::error::Error::source(&serve).is_some());
}

#[test]
fn a_stopped_thread_says_so() {
    assert_eq!(
        Error::ServerStopped.to_string(),
        "gateway thread stopped unexpectedly"
    );
}
