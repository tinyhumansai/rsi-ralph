//! The stored result of one sign test.

use serde::{Deserialize, Serialize};

use crate::ScoreDecision;

/// Outcome of [`super::sign_test`], persisted in run ledgers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignTest {
    /// Accept, reject or inconclusive.
    pub decision: ScoreDecision,
    /// Number of items both sides scored.
    pub items: usize,
    /// Items the candidate scored higher on.
    pub wins: u64,
    /// Items the candidate scored lower on.
    pub losses: u64,
    /// One-sided p-value of the candidate's wins.
    pub p: f64,
    /// Mean paired difference, positive meaning better.
    pub estimate: f64,
    /// Lower bound of the 90% interval around `estimate`.
    pub lower: f64,
    /// Upper bound of the 90% interval around `estimate`.
    pub upper: f64,
}
