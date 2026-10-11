//! The engine's ports and its stored ledger.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    Result,
    score::{Scorecard, Verdict},
    step::StepOutcome,
    subject::Checkout,
};

/// One epoch's request to the hive.
#[derive(Debug)]
pub struct ProposalRequest<'a> {
    /// Epoch number, from 1.
    pub epoch: u32,
    /// The rendered brief.
    pub brief: &'a str,
    /// The incumbent checkout, read-only for the hive.
    pub incumbent: &'a Checkout,
    /// Each editing seat's own candidate checkout.
    pub candidates: &'a BTreeMap<String, Checkout>,
    /// Folder for the hive's own records.
    pub dir: &'a Path,
}

/// What the hive reported.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalReport {
    /// The lead's closing summary, if it gave one.
    pub summary: Option<String>,
    /// Per-seat notes, such as how many turns each took.
    pub seats: BTreeMap<String, String>,
}

/// The agents that turn a brief into candidate changes: the hive.
pub trait Proposer {
    /// Seats that edit, each getting its own candidate checkout.
    fn editing_seats(&self) -> Vec<String>;

    /// Run the hive for one epoch; it edits the candidate folders in place.
    ///
    /// # Errors
    /// Returns an error when the hive cannot run at all.
    fn propose(&mut self, request: &ProposalRequest<'_>) -> Result<ProposalReport>;
}

/// The creative retreat that seeds a brief with ideas.
pub trait Ideator {
    /// Ideas for this epoch, given the brief rendered without them.
    ///
    /// # Errors
    /// Returns an error when the retreat cannot run.
    fn ideas(&mut self, epoch: u32, brief: &str, dir: &Path) -> Result<Vec<String>>;

    /// Record how an epoch's ideas fared, for memory across runs.
    ///
    /// # Errors
    /// Returns an error when the outcome cannot be stored.
    fn outcome(&mut self, epoch: u32, promoted: bool) -> Result<()>;
}

/// Model spend so far.
pub trait Meter {
    /// Dollars spent in this run.
    fn spent_usd(&self) -> f64;
    /// Dollars that may still be spent before a cap trips, if capped.
    fn remaining_usd(&self) -> Option<f64>;
}

/// Where a candidate ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    /// The hive changed nothing.
    Empty,
    /// The surface check or a gate failed.
    Gated,
    /// Skipped because the score reserve was not affordable.
    Unaffordable,
    /// Scored and not promoted.
    Scored,
    /// Scored, accepted and promoted.
    Promoted,
    /// Something failed while handling it.
    Error,
}

/// One candidate in `ledger.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateRecord {
    /// Candidate id, `e<epoch>-<seat>`.
    pub id: String,
    /// Epoch it was proposed in.
    pub epoch: u32,
    /// Seat that edited it.
    pub seat: String,
    /// Its checkout.
    pub checkout: PathBuf,
    /// Paths it changed.
    pub changed: Vec<String>,
    /// Where it ended up.
    pub status: CandidateStatus,
    /// Surface check and gate outcomes, in order.
    pub gates: Vec<StepOutcome>,
    /// Its scores.
    pub scorecard: Option<Scorecard>,
    /// The decision against the incumbent of its epoch.
    pub verdict: Option<Verdict>,
    /// Error message, scrubbed of key values.
    pub error: Option<String>,
}

/// One epoch in `ledger.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpochRecord {
    /// Epoch number.
    pub epoch: u32,
    /// Incumbent revision at the start of the epoch.
    pub incumbent: String,
    /// What the hive reported.
    pub report: ProposalReport,
    /// Retreat ideas used in the brief.
    pub ideas: Vec<String>,
    /// Candidate promoted this epoch.
    pub promoted: Option<String>,
}

/// The whole of `ledger.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    /// Incumbent revisions in promotion order, starting with the baseline.
    pub incumbents: Vec<String>,
    /// The current incumbent's scorecard.
    pub incumbent: Option<Scorecard>,
    /// Epochs in order.
    pub epochs: Vec<EpochRecord>,
    /// Every candidate in order.
    pub candidates: Vec<CandidateRecord>,
}
