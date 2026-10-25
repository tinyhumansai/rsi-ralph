//! A loop's score history. Field names are the form `rsi-ralph history
//! --json` and the UI's `/api/loops/<loop>/history` serve.

use serde::{Deserialize, Serialize};

/// Every measured score of a loop's stored runs, oldest run first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreHistory {
    /// Loop name.
    pub r#loop: String,
    /// One entry per run.
    pub runs: Vec<RunScores>,
    /// Each upgrade, with the score of the run before and after it.
    pub upgrades: Vec<UpgradeMark>,
}

/// One run's scores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunScores {
    /// Run id.
    pub run: String,
    /// Start, in Unix seconds.
    pub started_at: u64,
    /// Whether a larger deciding score is better.
    pub maximize: bool,
    /// The incumbent the run started from, scored at epoch 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<ScorePoint>,
    /// Epochs 1 and up, in order.
    pub epochs: Vec<EpochScores>,
}

/// One epoch of a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpochScores {
    /// Epoch number, from 1.
    pub epoch: u32,
    /// The incumbent revision it started from.
    pub incumbent: String,
    /// That incumbent's deciding score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incumbent_metric: Option<f64>,
    /// The incumbent's score when the epoch finished; unset while running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_metric: Option<f64>,
    /// Every candidate measured in it.
    pub candidates: Vec<ScorePoint>,
    /// Milliseconds from the epoch's start to the end of seat work (the
    /// retreat included): until its first candidate was considered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hive_ms: Option<u64>,
    /// Milliseconds from the end of seat work to `epoch_finished`: gates,
    /// scorers, decisions and promotion, with every seat idle. Unset while
    /// the epoch runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoring_ms: Option<u64>,
}

/// One measured checkout: the baseline or a candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScorePoint {
    /// Candidate id, such as `e1-chen-a`, or the incumbent's revision for a
    /// baseline.
    pub candidate: String,
    /// The seat that wrote it; unset for a baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seat: Option<String>,
    /// Mean deciding score over seeds; unset when no seed scored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean: Option<f64>,
    /// Seeds that failed.
    pub failures: u32,
    /// Seeds run.
    pub seeds: u32,
    /// Whether it was promoted.
    pub promoted: bool,
    /// Milliseconds into the run when it was measured.
    pub at_ms: u64,
}

/// An upgrade placed on the history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpgradeMark {
    /// Upgrade id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// The first run it applies to; unset while it waits for the next run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// The run before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<RunSummary>,
    /// The run it applies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<RunSummary>,
}

/// One run's scores in brief, for comparing across an upgrade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    /// Run id.
    pub run: String,
    /// The baseline score it started from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    /// Its best candidate score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best: Option<f64>,
    /// The incumbent score at its last finished epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
    /// Candidates measured.
    pub candidates: usize,
    /// Candidates promoted.
    pub promoted: usize,
}
