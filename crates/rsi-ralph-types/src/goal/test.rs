//! Serde pin and validation for goals.

#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]

use super::*;
use serde_json::json;

#[test]
fn a_goal_serializes_to_its_stored_form() {
    let goal = Goal::new("upgrade the file").with_criterion("no unwrap");
    assert_eq!(
        serde_json::to_value(&goal).unwrap(),
        json!({"statement": "upgrade the file", "criteria": ["no unwrap"]})
    );
    assert!(
        serde_json::from_value::<Goal>(json!({"statement": "s", "criteria": [], "x": 1})).is_err()
    );
}

#[test]
fn rejects_an_empty_statement_or_criterion() {
    assert!(Goal::new("ok").is_well_formed());
    assert!(!Goal::new("  ").is_well_formed());
    assert!(!Goal::new("ok").with_criterion(" ").is_well_formed());
}
