//! Unit tests for the Langfuse export error type.

use super::*;

#[test]
fn renders_lowercase_messages_without_secrets() {
    assert_eq!(
        Error::InvalidConfig.to_string(),
        "invalid langfuse export configuration"
    );
    assert_eq!(Error::ExportFailed.to_string(), "langfuse export failed");
}
