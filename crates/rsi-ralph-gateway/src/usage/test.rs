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

#[test]
fn notices_the_first_generated_token_and_not_keepalives() {
    let mut scanner = SseUsage::default();
    scanner.feed(b": OPENROUTER PROCESSING\n\n");
    scanner
        .feed(b"data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n");
    assert!(!scanner.saw_token());
    scanner.feed(b"data: {\"choices\":[{\"delta\":{\"reasoning\":\"Hm");
    assert!(
        !scanner.saw_token(),
        "an unterminated event is not read yet"
    );
    scanner.feed(b"\"}}]}\n\n");
    assert!(scanner.saw_token());
}

#[test]
fn counts_a_tool_call_delta_as_a_token() {
    let mut scanner = SseUsage::default();
    scanner.feed(
        b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"f\"}}]}}]}\n",
    );
    assert!(scanner.saw_token());
}

/// The usage block Surplus Intelligence returns, on a buffered response and
/// on the final chunk of a stream with `stream_options.include_usage`.
fn surplus_usage() -> serde_json::Value {
    json!({
        "prompt_tokens": 28831,
        "completion_tokens": 4,
        "total_tokens": 28835,
        "prompt_tokens_details": {"cached_tokens": 28800},
        "completion_tokens_details": {"reasoning_tokens": 0},
        "buyer_cost_micro": 11,
    })
}

#[test]
fn reads_cached_and_reasoning_tokens_from_a_buffered_response() {
    let usage = Usage::from_body(&json!({"choices": [], "usage": surplus_usage()}));
    assert_eq!((usage.prompt_tokens, usage.completion_tokens), (28831, 4));
    assert_eq!(usage.cached_tokens, Some(28800));
    assert_eq!(usage.reasoning_tokens, Some(0));
}

#[test]
fn reads_cached_and_reasoning_tokens_from_the_final_stream_chunk() {
    let mut scanner = SseUsage::default();
    scanner.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n");
    let last = json!({"choices": [], "usage": surplus_usage()});
    scanner.feed(format!("data: {last}\n\ndata: [DONE]\n\n").as_bytes());
    let usage = scanner.finish();
    assert_eq!(usage.cached_tokens, Some(28800));
    assert_eq!(usage.reasoning_tokens, Some(0));
}

#[test]
fn leaves_cached_and_reasoning_tokens_unknown_without_details() {
    let missing =
        Usage::from_body(&json!({"usage": {"prompt_tokens": 10, "completion_tokens": 3}}));
    assert_eq!(
        (missing.cached_tokens, missing.reasoning_tokens),
        (None, None)
    );
    let null = Usage::from_body(&json!({"usage": {
        "prompt_tokens": 10,
        "prompt_tokens_details": {"cached_tokens": null},
        "completion_tokens_details": null,
    }}));
    assert_eq!((null.cached_tokens, null.reasoning_tokens), (None, None));
}

#[test]
fn ignores_cache_and_reasoning_counts_when_pricing_at_the_ceiling() {
    let mut body = json!({"usage": surplus_usage()});
    let plain =
        Usage::from_body(&json!({"usage": {"prompt_tokens": 28831, "completion_tokens": 4}}));
    body["usage"]["prompt_tokens_details"]["cached_tokens"] = json!(28831);
    let price = PriceTable::standard().get(GLM_FLASH).unwrap().clone();
    assert_eq!(
        Usage::from_body(&body).cost_or_ceiling(&price),
        plain.cost_or_ceiling(&price)
    );
}
