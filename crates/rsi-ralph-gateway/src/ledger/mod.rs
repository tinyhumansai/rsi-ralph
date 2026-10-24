//! Durable spend ledgers and the two-cap budget every call reserves against.
//!
//! A [`Ledger`] is one JSON file holding a fixed dollar limit and what has
//! been charged against it. [`Budgets`] pairs the per-run ledger with the
//! campaign-wide total ledger: a call proceeds only if its worst-case cost
//! fits in **both**, and the reservation is held in both until the call
//! settles.
//!
//! Charging is conservative:
//!
//! - a settled call is charged its reported cost;
//! - a call whose cost is unknown keeps its whole reservation (`uncertain`);
//! - a reservation still in flight when the process dies is converted to
//!   `uncertain` the next time the ledger opens;
//! - a reported cost above its reservation sets `breached`, which refuses
//!   every later call until a human looks at it.
//!
//! The limit of an existing ledger can never change.

mod types;

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub(crate) use types::{LedgerState, Reservation};

/// One ledger file.
#[derive(Debug)]
pub(crate) struct Ledger {
    path: PathBuf,
    state: LedgerState,
}

impl Ledger {
    /// Opens the ledger at `path`, creating it with `limit_usd` if absent.
    ///
    /// Reservations left in flight by a previous process become uncertain.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLimit`] for a non-positive or non-finite limit,
    /// [`Error::LimitChanged`] if the file holds a different limit,
    /// [`Error::LedgerFormat`] if it is not a ledger, or [`Error::Io`].
    pub(crate) fn open(path: &Path, limit_usd: f64) -> Result<Self> {
        if !limit_usd.is_finite() || limit_usd <= 0.0 {
            return Err(Error::InvalidLimit(limit_usd));
        }
        let state = match std::fs::read(path) {
            Ok(bytes) => {
                let mut state: LedgerState =
                    serde_json::from_slice(&bytes).map_err(|error| Error::LedgerFormat {
                        path: path.to_path_buf(),
                        message: error.to_string(),
                    })?;
                if (state.limit_usd - limit_usd).abs() > 1e-9 {
                    return Err(Error::LimitChanged {
                        path: path.to_path_buf(),
                        stored: state.limit_usd,
                        requested: limit_usd,
                    });
                }
                state.uncertain_usd += state.in_flight_usd;
                state.in_flight_usd = 0.0;
                state
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                LedgerState::new(limit_usd)
            }
            Err(source) => {
                return Err(Error::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        let ledger = Self {
            path: path.to_path_buf(),
            state,
        };
        ledger.persist()?;
        Ok(ledger)
    }

    /// The current state.
    pub(crate) fn state(&self) -> &LedgerState {
        &self.state
    }

    fn persist(&self) -> Result<()> {
        let io = |source| Error::Io {
            path: self.path.clone(),
            source,
        };
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let mut text =
            serde_json::to_string_pretty(&self.state).map_err(|error| Error::LedgerFormat {
                path: self.path.clone(),
                message: error.to_string(),
            })?;
        text.push('\n');
        std::fs::write(&tmp, text).map_err(io)?;
        std::fs::rename(&tmp, &self.path).map_err(io)
    }
}

/// The per-run and total ledgers, reserved and settled together.
#[derive(Debug)]
pub(crate) struct Budgets {
    run: Ledger,
    total: Ledger,
}

impl Budgets {
    /// Pairs a run ledger with the total ledger.
    pub(crate) fn new(run: Ledger, total: Ledger) -> Self {
        Self { run, total }
    }

    /// Admits a call that may cost up to `amount` dollars.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PriceBoundBreached`] once any ledger is breached,
    /// [`Error::BudgetExceeded`] naming the first ledger the call does not
    /// fit in, or [`Error::Io`] if the ledgers cannot be saved.
    pub(crate) fn reserve(&mut self, amount: f64) -> Result<Reservation> {
        let amount = if amount.is_finite() {
            amount.max(0.0)
        } else {
            f64::MAX
        };
        if self.run.state.breached || self.total.state.breached {
            return Err(Error::PriceBoundBreached);
        }
        let remaining = [
            ("run", self.run.state.remaining_usd()),
            ("total", self.total.state.remaining_usd()),
        ];
        for (name, remaining) in remaining {
            if amount > remaining {
                self.run.state.denied += 1;
                self.total.state.denied += 1;
                self.persist()?;
                return Err(Error::BudgetExceeded {
                    ledger: name,
                    needed: amount,
                    remaining,
                });
            }
        }
        for ledger in [&mut self.run, &mut self.total] {
            ledger.state.in_flight_usd += amount;
            ledger.state.calls += 1;
        }
        self.persist()?;
        Ok(Reservation::new(amount))
    }

    /// Settles a call with its actual cost, or `None` if it is unknown.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the ledgers cannot be saved.
    pub(crate) fn settle(&mut self, reservation: Reservation, cost: Option<f64>) -> Result<()> {
        let amount = reservation.amount();
        for ledger in [&mut self.run, &mut self.total] {
            let state = &mut ledger.state;
            state.in_flight_usd = (state.in_flight_usd - amount).max(0.0);
            match cost {
                Some(cost) if cost.is_finite() && cost >= 0.0 => {
                    state.settled_usd += cost;
                    if cost > amount + 1e-12 {
                        state.breached = true;
                    }
                }
                _ => state.uncertain_usd += amount,
            }
        }
        self.persist()
    }

    /// The run and total ledger states, in that order.
    pub(crate) fn states(&self) -> (&LedgerState, &LedgerState) {
        (self.run.state(), self.total.state())
    }

    fn persist(&self) -> Result<()> {
        self.run.persist()?;
        self.total.persist()
    }
}

#[cfg(test)]
mod test;
