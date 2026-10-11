//! Unit tests for the scoring error type.

use super::*;

#[test]
fn keeps_the_messages_callers_already_see() {
    assert_eq!(
        Error::InvalidEvaluation.to_string(),
        "invalid evaluation data or policy"
    );
    assert_eq!(Error::HostFailed.to_string(), "host operation failed");
}
