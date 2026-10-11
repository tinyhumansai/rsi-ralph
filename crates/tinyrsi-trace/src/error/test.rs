//! Unit tests for the trace error type.

use super::*;

#[test]
fn renders_lowercase_messages() {
    assert_eq!(Error::Io.to_string(), "loop trace write failed");
    assert_eq!(
        Error::InvalidExporter.to_string(),
        "invalid trace exporter configuration"
    );
}
