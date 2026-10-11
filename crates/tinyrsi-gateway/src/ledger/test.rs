//! Unit tests for ledgers and budgets.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use super::{Budgets, Ledger};
use crate::error::Error;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("ohrsi-gw-{}-ledger-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn budgets(&self, run: f64, total: f64) -> Budgets {
        Budgets::new(
            Ledger::open(&self.0.join("run/budget.json"), run).unwrap(),
            Ledger::open(&self.0.join("total.json"), total).unwrap(),
        )
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rejects_invalid_limits() {
    let scratch = Scratch::new("invalid");
    for limit in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let error = Ledger::open(&scratch.0.join("x.json"), limit).unwrap_err();
        assert!(matches!(error, Error::InvalidLimit(_)), "{limit}");
    }
}

#[test]
fn settles_to_the_reported_cost() {
    let scratch = Scratch::new("settle");
    let mut budgets = scratch.budgets(5.0, 50.0);
    let reservation = budgets.reserve(0.2).unwrap();
    assert!((budgets.states().0.in_flight_usd - 0.2).abs() < 1e-12);
    budgets.settle(reservation, Some(0.05)).unwrap();
    let (run, total) = budgets.states();
    assert!((run.charged_usd() - 0.05).abs() < 1e-12);
    assert!((total.settled_usd - 0.05).abs() < 1e-12);
    assert_eq!(run.calls, 1);
    assert!(!run.breached);
}

#[test]
fn unknown_costs_keep_the_whole_reservation() {
    let scratch = Scratch::new("unknown");
    let mut budgets = scratch.budgets(5.0, 50.0);
    let reservation = budgets.reserve(0.3).unwrap();
    assert!((reservation.amount() - 0.3).abs() < 1e-12);
    budgets.settle(reservation, None).unwrap();
    assert!((budgets.states().0.uncertain_usd - 0.3).abs() < 1e-12);
    let reservation = budgets.reserve(0.1).unwrap();
    budgets.settle(reservation, Some(f64::NAN)).unwrap();
    assert!((budgets.states().1.uncertain_usd - 0.4).abs() < 1e-12);
}

#[test]
fn refuses_a_call_that_does_not_fit_the_run_cap() {
    let scratch = Scratch::new("run-cap");
    let mut budgets = scratch.budgets(1.0, 50.0);
    let held = budgets.reserve(0.9).unwrap();
    let error = budgets.reserve(0.2).unwrap_err();
    assert!(
        matches!(error, Error::BudgetExceeded { ledger: "run", .. }),
        "{error}"
    );
    assert_eq!(budgets.states().0.denied, 1);
    budgets.settle(held, Some(0.1)).unwrap();
    let reservation = budgets.reserve(0.2).unwrap();
    budgets.settle(reservation, Some(0.2)).unwrap();
}

#[test]
fn refuses_a_call_that_does_not_fit_the_total_cap() {
    let scratch = Scratch::new("total-cap");
    let mut budgets = scratch.budgets(5.0, 0.5);
    let error = budgets.reserve(0.6).unwrap_err();
    assert!(
        matches!(
            error,
            Error::BudgetExceeded {
                ledger: "total",
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn a_breached_bound_halts_all_spending() {
    let scratch = Scratch::new("breach");
    let mut budgets = scratch.budgets(5.0, 50.0);
    let reservation = budgets.reserve(0.01).unwrap();
    budgets.settle(reservation, Some(0.02)).unwrap();
    assert!(budgets.states().0.breached);
    assert!(matches!(
        budgets.reserve(0.0).unwrap_err(),
        Error::PriceBoundBreached
    ));
}

#[test]
fn non_finite_reservations_never_fit() {
    let scratch = Scratch::new("nonfinite");
    let mut budgets = scratch.budgets(5.0, 50.0);
    assert!(matches!(
        budgets.reserve(f64::NAN).unwrap_err(),
        Error::BudgetExceeded { .. }
    ));
}

#[test]
fn reopening_carries_spend_and_makes_in_flight_uncertain() {
    let scratch = Scratch::new("reopen");
    {
        let mut budgets = scratch.budgets(5.0, 50.0);
        let reservation = budgets.reserve(0.1).unwrap();
        budgets.settle(reservation, Some(0.1)).unwrap();
        let _abandoned = budgets.reserve(0.25).unwrap();
    }
    let budgets = scratch.budgets(5.0, 50.0);
    let (run, total) = budgets.states();
    assert!((run.settled_usd - 0.1).abs() < 1e-12);
    assert!((run.uncertain_usd - 0.25).abs() < 1e-12);
    assert!(total.in_flight_usd.abs() < 1e-12);
}

#[test]
fn the_limit_of_an_existing_ledger_cannot_change() {
    let scratch = Scratch::new("limit");
    let path = scratch.0.join("total.json");
    Ledger::open(&path, 50.0).unwrap();
    let error = Ledger::open(&path, 60.0).unwrap_err();
    assert!(matches!(error, Error::LimitChanged { .. }), "{error}");
}

#[test]
fn rejects_a_corrupt_ledger() {
    let scratch = Scratch::new("corrupt");
    let path = scratch.0.join("total.json");
    std::fs::write(&path, "not json").unwrap();
    assert!(matches!(
        Ledger::open(&path, 50.0).unwrap_err(),
        Error::LedgerFormat { .. }
    ));
}

#[test]
fn reports_unreadable_ledgers() {
    let scratch = Scratch::new("unreadable");
    let error = Ledger::open(&scratch.0, 50.0).unwrap_err();
    assert!(matches!(error, Error::Io { .. }), "{error}");
}
