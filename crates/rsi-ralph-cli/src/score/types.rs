//! What scoring one checkout produced, and the verdict on a candidate.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use rsi_ralph_score::{ScoreDecision, SignTest};
use rsi_ralph_trace::IntervalUnits;

use crate::step::ScoreResult;

/// One run of one scorer on one seed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeedScore {
    /// The seed.
    pub seed: u64,
    /// Whether the command succeeded and reported a score.
    pub ok: bool,
    /// Why it failed.
    pub reason: Option<String>,
    /// What it reported.
    pub result: Option<ScoreResult>,
    /// Wall-clock milliseconds.
    pub duration_ms: u64,
}

/// A judge's grade of a candidate's change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgeScore {
    /// Quality in `[0, 1]`.
    pub quality: f64,
    /// The judge's reasoning.
    pub rationale: Option<String>,
}

/// Everything scoring one checkout produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    /// `incumbent` or the candidate id.
    pub label: String,
    /// Run scorers by id, one entry per seed.
    pub runs: BTreeMap<String, Vec<SeedScore>>,
    /// Judge scorers by id.
    pub judges: BTreeMap<String, JudgeScore>,
    /// Mean of the deciding scorer, when every seed succeeded.
    pub headline: Option<f64>,
}

/// The decision on one candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    /// Accept, reject or inconclusive.
    pub decision: ScoreDecision,
    /// Improvement estimate, positive meaning better.
    pub estimate: f64,
    /// Lower interval bound.
    pub lower: f64,
    /// Upper interval bound.
    pub upper: f64,
    /// Units of the interval.
    pub units: IntervalUnits,
    /// Sign-test details, for the `sign` rule.
    pub sign: Option<SignTest>,
    /// Why, in a sentence.
    pub reason: String,
}
