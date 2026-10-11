//! Integration tests for the public crate surface.
//!
//! These tests link against the crate as a downstream consumer would: they can
//! only use what `src/lib.rs` re-exports. Treat them as the regression suite
//! for the crate's public contract — if a change breaks a test here, it is a
//! breaking change for users.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use tinyrsi::{Error, GradeEntry, Lineage, Verdict};

fn entry(step: u64, grade: f64, verdict: Verdict) -> GradeEntry {
    GradeEntry::new(step, "sha256:abc", grade, verdict, "rationale")
}

#[test]
fn the_ledger_is_available_to_consumers() {
    let mut lineage = Lineage::new();
    assert!(lineage.append(entry(0, 0.703, Verdict::Accepted)).unwrap());
    assert_eq!(lineage.len(), 1);
}

#[test]
fn errors_are_available_to_consumers() {
    let mut lineage = Lineage::new();
    assert_eq!(
        lineage
            .append(entry(3, 0.5, Verdict::Accepted))
            .unwrap_err(),
        Error::StepOutOfOrder
    );
}

#[test]
fn the_head_is_available_to_consumers() {
    let mut lineage = Lineage::new();
    lineage.append(entry(0, 0.703, Verdict::Accepted)).unwrap();
    lineage.append(entry(1, 0.778, Verdict::Accepted)).unwrap();
    assert_eq!(lineage.head().map(|e| e.grade), Some(0.778));
}
