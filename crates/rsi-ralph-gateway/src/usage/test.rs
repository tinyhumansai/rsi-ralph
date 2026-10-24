//! Unit tests for usage parsing.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use super::{SseUsage, Usage};
use crate::price::{GLM_FLASH, PriceTable};

#[test]
fn reads_reported_cost() {
    let usage = Usage::from_body(
        &json!({"usage": {"cost": 0.0012, "prompt_tokens": 10, "completion_tokens": 3}}),
    );
    assert_eq!(usage.cost, Some(0.0012));
    assert_eq!((usage.prompt_tokens, usage.completion_tokens), (10, 3));
    let price = PriceTable::standard().get(GLM_FLASH).unwrap().clone();
    assert_eq!(usage.cost_or_ceiling(&price), Some(0.0012));
}

#[test]
fn prices_tokens_at_the_ceiling_without_a_cost() {
    let usage = Usage::from_body(
        &json!({"usage": {"prompt_tokens": 1_000_000, "completion_tokens": 1_000_000}}),
    );
    let price = PriceTable::standard().get(GLM_FLASH).unwrap().clone();
    assert!((usage.cost_or_ceiling(&price).unwrap() - 1.30).abs() < 1e-9);
    assert_eq!(Usage::from_body(&json!({})).cost_or_ceiling(&price), None);
}

#[test]
fn finds_usage_split_across_stream_chunks() {
    let mut scanner = SseUsage::default();
    scanner.feed(b"data: {\"choices\":[]}\n\ndata: {\"usage\":{\"co");
    scanner.feed(b"st\":0.5,\"prompt_tokens\":4}}\n\n");
    scanner.feed(b"data: [DONE]\n\n");
    let usage = scanner.finish();
    assert_eq!(usage.cost, Some(0.5));
    assert_eq!(usage.prompt_tokens, 4);
}

#[test]
fn reads_a_final_unterminated_event_and_skips_noise() {
    let mut scanner = SseUsage::default();
    scanner.feed(b": keepalive\n\xff\xfe\n");
    scanner.feed(b"data: {\"usage\":{\"cost\":0.1}}");
    assert_eq!(scanner.finish().cost, Some(0.1));
}
