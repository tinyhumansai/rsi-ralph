//! Unit tests for the price table.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{EMBEDDING, GLM_FLASH, Price, PriceTable, usd};
use crate::error::Error;

#[test]
fn allows_exactly_the_chat_and_embedding_models() {
    let table = PriceTable::standard();
    assert_eq!(table.models().collect::<Vec<_>>(), [EMBEDDING, GLM_FLASH]);
    assert_eq!(table.get(GLM_FLASH).unwrap().provider, "z-ai");
}

#[test]
fn refuses_unknown_models() {
    let error = PriceTable::standard().get("openai/gpt-5.2").unwrap_err();
    assert!(matches!(error, Error::UnknownModel(_)));
}

#[test]
fn an_empty_table_allows_only_what_is_inserted() {
    let mut table = PriceTable::new();
    assert_eq!(table.models().count(), 0);
    table.insert(
        "x/y",
        Price {
            prompt: 1e-6,
            completion: 2e-6,
            provider: "x".into(),
        },
    );
    assert_eq!(table.get("x/y").unwrap().provider, "x");
}

#[test]
fn saturates_absurd_token_counts() {
    assert!((usd(1_000_000, 1e-6) - 1.0).abs() < 1e-12);
    assert!((usd(u64::MAX, 1.0) - f64::from(u32::MAX)).abs() < 1e-3);
}
