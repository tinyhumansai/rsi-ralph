//! Unit tests for request preparation.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use super::{Endpoint, Limits, PROMPT_SLACK_TOKENS, prepare};
use crate::error::Error;
use crate::price::{EMBEDDING, GLM_FLASH, PriceTable, usd};

fn chat(body: serde_json::Value) -> Result<super::Prepared, Error> {
    prepare(
        Endpoint::Chat,
        body,
        &PriceTable::standard(),
        Limits::default(),
    )
}

#[test]
fn pins_the_provider_and_price_ceiling() {
    let prepared =
        chat(json!({"model": GLM_FLASH, "messages": [{"role": "user", "content": "hi"}]})).unwrap();
    assert_eq!(prepared.body["provider"]["only"], json!(["z-ai"]));
    assert_eq!(prepared.body["provider"]["allow_fallbacks"], json!(false));
    assert!(
        (prepared.body["provider"]["max_price"]["completion"]
            .as_f64()
            .unwrap()
            - 1.0)
            .abs()
            < 1e-9
    );
    assert_eq!(prepared.body["usage"], json!({"include": true}));
}

#[test]
fn defaults_and_clamps_max_tokens() {
    let prepared = chat(json!({"model": GLM_FLASH, "messages": []})).unwrap();
    assert_eq!(prepared.body["max_tokens"], json!(8192));
    let prepared =
        chat(json!({"model": GLM_FLASH, "messages": [], "max_tokens": 999_999})).unwrap();
    assert_eq!(prepared.body["max_tokens"], json!(16384));
    let prepared =
        chat(json!({"model": GLM_FLASH, "messages": [], "max_completion_tokens": 100})).unwrap();
    assert_eq!(prepared.body["max_tokens"], json!(100));
    assert!(prepared.body.get("max_completion_tokens").is_none());
}

#[test]
fn reserves_the_worst_case_cost() {
    let body = json!({"model": GLM_FLASH, "messages": [], "max_tokens": 1000});
    let bytes = serde_json::to_vec(&body).unwrap().len() as u64;
    let prepared = chat(body).unwrap();
    let expected = usd(bytes + PROMPT_SLACK_TOKENS, 0.30e-6) + usd(1000, 1.0e-6);
    assert!((prepared.reserve_usd - expected).abs() < 1e-12);
    assert!(!prepared.stream);
}

#[test]
fn asks_streams_to_report_usage() {
    let prepared = chat(json!({"model": GLM_FLASH, "messages": [], "stream": true})).unwrap();
    assert!(prepared.stream);
    assert_eq!(
        prepared.body["stream_options"],
        json!({"include_usage": true})
    );
}

#[test]
fn accepts_text_parts_and_rejects_images() {
    let text = json!({"model": GLM_FLASH, "messages": [{"role": "user", "content": [{"type": "text", "text": "a"}]}]});
    chat(text).unwrap();
    let image = json!({"model": GLM_FLASH, "messages": [{"role": "user", "content": [{"type": "image_url", "image_url": {"url": "data:"}}]}]});
    assert!(matches!(chat(image).unwrap_err(), Error::BadRequest(_)));
}

#[test]
fn rejects_malformed_requests() {
    assert!(matches!(chat(json!([])).unwrap_err(), Error::BadRequest(_)));
    assert!(matches!(
        chat(json!({"messages": []})).unwrap_err(),
        Error::BadRequest(_)
    ));
    assert!(matches!(
        chat(json!({"model": GLM_FLASH})).unwrap_err(),
        Error::BadRequest(_)
    ));
    assert!(matches!(
        chat(json!({"model": "x/y", "messages": []})).unwrap_err(),
        Error::UnknownModel(_)
    ));
}

#[test]
fn embeddings_reserve_prompt_side_only() {
    let prepared = prepare(
        Endpoint::Embeddings,
        json!({"model": EMBEDDING, "input": "hello"}),
        &PriceTable::standard(),
        Limits::default(),
    )
    .unwrap();
    assert!(prepared.body.get("max_tokens").is_none());
    assert_eq!(prepared.body["provider"]["only"], json!(["openai"]));
    assert!(prepared.reserve_usd < 0.001);
    assert_eq!(Endpoint::Embeddings.path(), "embeddings");
    assert_eq!(Endpoint::Chat.path(), "chat/completions");
}

#[test]
fn pins_reasoning_effort_unless_the_client_chose() {
    let prepared = chat(json!({"model": GLM_FLASH, "messages": []})).unwrap();
    assert_eq!(prepared.body["reasoning"], json!({"effort": "low"}));
    let prepared =
        chat(json!({"model": GLM_FLASH, "messages": [], "reasoning_effort": "high"})).unwrap();
    assert_eq!(prepared.body["reasoning"], json!({"effort": "high"}));
    assert!(prepared.body.get("reasoning_effort").is_none());
    let prepared =
        chat(json!({"model": GLM_FLASH, "messages": [], "reasoning": {"enabled": false}})).unwrap();
    assert_eq!(prepared.body["reasoning"], json!({"enabled": false}));
    let none = Limits {
        default_reasoning: None,
        ..Limits::default()
    };
    let prepared = prepare(
        Endpoint::Chat,
        json!({"model": GLM_FLASH, "messages": []}),
        &PriceTable::standard(),
        none,
    )
    .unwrap();
    assert!(prepared.body.get("reasoning").is_none());
}
