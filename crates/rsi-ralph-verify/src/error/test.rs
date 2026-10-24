//! Unit tests for the verification error type.

use super::*;

#[test]
fn keeps_the_messages_callers_already_see() {
    assert_eq!(
        Error::InvalidBlueprint.to_string(),
        "invalid blueprint or verification outcome"
    );
    assert_eq!(Error::HostFailed.to_string(), "host operation failed");
}
