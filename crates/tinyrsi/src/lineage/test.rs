//! Unit tests for the lineage module.
//!
//! Unit tests live next to the code they cover and may reach into private
//! items. Tests of the public contract belong in `tests/` instead.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use tinyrsi_bus::Verdict;

fn entry(step: u64, grade: f64, verdict: Verdict) -> GradeEntry {
    GradeEntry::new(step, "sha256:abc", grade, verdict, "rationale")
}

#[test]
fn an_empty_ledger_has_no_head() {
    let lineage = Lineage::new();
    assert!(lineage.is_empty());
    assert_eq!(lineage.len(), 0);
    assert!(lineage.head().is_none());
}

#[test]
fn records_an_entry_whose_step_continues_the_ledger() {
    let mut lineage = Lineage::new();
    assert!(lineage.append(entry(0, 0.703, Verdict::Accepted)).unwrap());
    assert_eq!(lineage.len(), 1);
    assert_eq!(lineage.entries()[0].step, 0);
}

#[test]
fn rejects_a_step_that_skips_ahead() {
    let mut lineage = Lineage::new();
    assert_eq!(
        lineage
            .append(entry(1, 0.7, Verdict::Accepted))
            .unwrap_err(),
        Error::StepOutOfOrder
    );
    assert!(lineage.is_empty());
}

#[test]
fn rejects_a_replayed_step() {
    let mut lineage = Lineage::new();
    lineage.append(entry(0, 0.703, Verdict::Accepted)).unwrap();
    assert_eq!(
        lineage
            .append(entry(0, 0.778, Verdict::Rejected))
            .unwrap_err(),
        Error::StepOutOfOrder
    );
    assert_eq!(lineage.len(), 1);
}

#[test]
fn rejects_an_entry_without_a_snapshot() {
    let mut lineage = Lineage::new();
    let entry = GradeEntry::new(0, "", 0.5, Verdict::Accepted, "rationale");
    assert_eq!(lineage.append(entry).unwrap_err(), Error::EmptyField);
}

#[test]
fn rejects_an_entry_without_a_rationale() {
    let mut lineage = Lineage::new();
    let entry = GradeEntry::new(0, "sha256:abc", 0.5, Verdict::Accepted, "  ");
    assert_eq!(lineage.append(entry).unwrap_err(), Error::EmptyField);
}

#[test]
fn the_head_is_the_last_accepted_entry() {
    let mut lineage = Lineage::new();
    lineage.append(entry(0, 0.703, Verdict::Accepted)).unwrap();
    lineage.append(entry(1, 0.5, Verdict::Rejected)).unwrap();
    lineage.append(entry(2, 0.6, Verdict::Rejected)).unwrap();
    lineage.append(entry(3, 0.778, Verdict::Accepted)).unwrap();
    assert_eq!(lineage.head().map(|e| e.step), Some(3));
}

#[test]
fn a_rejected_tail_does_not_move_the_head() {
    let mut lineage = Lineage::new();
    lineage.append(entry(0, 0.703, Verdict::Accepted)).unwrap();
    lineage.append(entry(1, 0.9, Verdict::Rejected)).unwrap();
    assert_eq!(lineage.head().map(|e| e.step), Some(0));
}

#[test]
fn entries_stay_in_step_order() {
    let mut lineage = Lineage::new();
    for step in 0..4 {
        lineage.append(entry(step, 0.5, Verdict::Rejected)).unwrap();
    }
    let steps: Vec<u64> = lineage.entries().iter().map(|e| e.step).collect();
    assert_eq!(steps, vec![0, 1, 2, 3]);
}
