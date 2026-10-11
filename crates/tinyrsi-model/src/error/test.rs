//! Unit tests for the model error type.

use super::*;

#[test]
fn renders_lowercase_messages_without_payloads() {
    assert_eq!(
        Error::InvalidRequest.to_string(),
        "invalid model request, limit or reply"
    );
    assert_eq!(Error::CallFailed.to_string(), "model call failed");
}

#[test]
fn is_a_standard_error() {
    fn assert_error<E: std::error::Error>(_: &E) {}
    assert_error(&Error::CallFailed);
}
