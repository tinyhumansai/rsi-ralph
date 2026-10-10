//! Unit tests for the crate-wide error type.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

#[test]
fn renders_a_human_readable_message_for_an_empty_field() {
    assert_eq!(
        Error::EmptyField.to_string(),
        "lineage entry has an empty snapshot address or rationale"
    );
}

#[test]
fn renders_a_human_readable_message_for_an_out_of_order_step() {
    assert_eq!(
        Error::StepOutOfOrder.to_string(),
        "lineage entry's step does not continue the ledger"
    );
}

#[test]
fn is_a_standard_error() {
    fn assert_error<E: std::error::Error>(_: &E) {}

    assert_error(&Error::EmptyField);
    assert_error(&Error::StepOutOfOrder);
}
