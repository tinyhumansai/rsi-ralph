//! Unit tests for reading limits out of an upstream model listing.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use super::{entry, limits_by_id};

#[test]
fn only_limit_fields_are_kept_and_entries_without_any_are_dropped() {
    let body = json!({"data": [
        {"id": "flash", "context_length": 262_144, "rungs": 8, "pricing": {"prompt": "1"}},
        {"id": "vectors", "rungs": 2},
        {"id": "z-ai/glm", "top_provider": {"context_length": 128_000, "max_completion_tokens": 8192}},
        {"context_length": 5},
    ]});
    let limits = limits_by_id(&body);
    assert_eq!(limits.len(), 2);
    assert_eq!(limits["flash"].get("context_length"), Some(&json!(262_144)));
    assert!(limits["flash"].get("pricing").is_none());
    assert!(limits["flash"].get("rungs").is_none());
    assert_eq!(
        limits["z-ai/glm"]["top_provider"]["context_length"],
        128_000
    );
}

#[test]
fn a_body_that_is_not_a_listing_has_no_limits() {
    assert!(limits_by_id(&json!({"error": "nope"})).is_empty());
    assert!(limits_by_id(&json!([])).is_empty());
}

#[test]
fn an_entry_keeps_its_id_and_object_whatever_the_upstream_says() {
    let mut limits = serde_json::Map::new();
    limits.insert("context_length".into(), json!(1000));
    assert_eq!(
        entry("flash", Some(&limits)),
        json!({"id": "flash", "object": "model", "context_length": 1000})
    );
    assert_eq!(
        entry("bare", None),
        json!({"id": "bare", "object": "model"})
    );
}
