//! The persisted ledger state and the hold an admitted call carries.

use serde::{Deserialize, Serialize};

/// The persisted state of one ledger; its JSON form is the ledger file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct LedgerState {
    /// The fixed spend limit in dollars.
    pub(crate) limit_usd: f64,
    /// Sum of settled costs, reported or computed.
    pub(crate) settled_usd: f64,
    /// Sum of reservations kept because the real cost is unknown.
    pub(crate) uncertain_usd: f64,
    /// Sum of reservations for calls still running.
    pub(crate) in_flight_usd: f64,
    /// Calls that were admitted.
    pub(crate) calls: u64,
    /// Calls refused because they did not fit.
    pub(crate) denied: u64,
    /// Whether a call ever cost more than its reservation.
    pub(crate) breached: bool,
}

impl LedgerState {
    /// A fresh ledger with nothing charged against `limit_usd`.
    pub(crate) fn new(limit_usd: f64) -> Self {
        Self {
            limit_usd,
            settled_usd: 0.0,
            uncertain_usd: 0.0,
            in_flight_usd: 0.0,
            calls: 0,
            denied: 0,
            breached: false,
        }
    }

    /// Everything counted against the limit right now.
    pub(crate) fn charged_usd(&self) -> f64 {
        self.settled_usd + self.uncertain_usd + self.in_flight_usd
    }

    /// What is left before the limit.
    pub(crate) fn remaining_usd(&self) -> f64 {
        (self.limit_usd - self.charged_usd()).max(0.0)
    }
}

/// An admitted call's hold on both ledgers.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use = "a reservation must be settled"]
pub(crate) struct Reservation {
    amount: f64,
}

impl Reservation {
    /// Holds `amount` dollars.
    pub(crate) fn new(amount: f64) -> Self {
        Self { amount }
    }

    /// The dollars held.
    pub(crate) fn amount(self) -> f64 {
        self.amount
    }
}
