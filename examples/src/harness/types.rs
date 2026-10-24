//! The records and summary the benchmark runner writes.
use rsi_ralph::StopReason;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Which cell of the matrix a run belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCell {
    /// Benchmark name.
    pub benchmark: String,
    /// Model label (`offline`, or a sanitized `OpenRouter` id).
    pub model: String,
    /// Repeat number, from 1.
    pub repeat: u32,
}

/// The measured outcome of one run, as `summary.json` stores it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Benchmark name.
    pub benchmark: String,
    /// Model label.
    pub model: String,
    /// Repeat number, from 1.
    pub repeat: u32,
    /// Why the run ended; `error` when it failed or wrote no stop reason.
    pub stop: StopReason,
    /// True when the run stopped with its goal reached.
    pub goal_reached: bool,
    /// Highest epoch with a ledger row.
    pub epochs: u64,
    /// Metric name, empty without a ledger.
    pub metric: String,
    /// Baseline mean metric, absent without a measured baseline.
    pub baseline_metric: Option<f64>,
    /// Final incumbent mean metric, absent without a measured baseline.
    pub final_metric: Option<f64>,
    /// Candidates promoted to incumbent.
    pub promotions: u32,
    /// Proposals (ledger rows after the baseline).
    pub proposals: u32,
    /// Proposals not accepted, including duplicates and invalid ones.
    pub rejected: u32,
    /// Proposals whose content was already evaluated in the run.
    pub duplicates: u32,
    /// Proposals refused before running.
    pub invalid: u32,
    /// Proposals repeating an earlier ruled-out one.
    pub repeats: u32,
    /// `repeats / proposals`, absent without proposals.
    pub repetition_share: Option<f64>,
    /// Measured supervisor and scorer tokens.
    pub tokens: u64,
    /// Wall time of the run, in milliseconds.
    pub wall_ms: u64,
    /// The run's directory.
    pub run_dir: PathBuf,
    /// The run's error message, absent on success.
    pub error: Option<String>,
}

/// One benchmark × model row of the comparison table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryRow {
    /// Benchmark name.
    pub benchmark: String,
    /// Model label.
    pub model: String,
    /// Metric name, from the first run with a ledger.
    pub metric: String,
    /// Runs in the cell.
    pub runs: u32,
    /// Runs that reached the goal.
    pub goals: u32,
    /// `goals / runs`.
    pub goal_rate: f64,
    /// Median epochs over the runs that reached the goal.
    pub median_epochs_to_goal: Option<f64>,
    /// Median tokens over all runs.
    pub median_tokens: Option<f64>,
    /// Median final metric over runs with one.
    pub median_final_metric: Option<f64>,
    /// Mean repetition share over runs with one.
    pub mean_repetition_share: Option<f64>,
    /// Median wall time over all runs, in milliseconds.
    pub median_wall_ms: Option<f64>,
}

/// The `summary.json` document: aggregate rows, then every run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// One row per benchmark × model, in matrix order.
    pub rows: Vec<SummaryRow>,
    /// Every run, in matrix order.
    pub runs: Vec<RunRecord>,
}
