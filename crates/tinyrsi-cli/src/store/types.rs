//! `run.json`, the per-run status record. Field names are the stored form.

use serde::{Deserialize, Serialize};
use tinyrsi_trace::StopReason;

/// Lifecycle of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// The run's process is working on it.
    Running,
    /// The loop stopped on a stop reason.
    Finished,
    /// The loop stopped on an error.
    Failed,
    /// The process died while the run was marked running.
    Interrupted,
}

/// The contents of `run.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Run id, unique within the loop.
    pub id: String,
    /// Loop name.
    pub r#loop: String,
    /// Lifecycle state.
    pub status: RunStatus,
    /// Process that owns a running run.
    pub pid: u32,
    /// Start, in Unix seconds.
    pub started_at: u64,
    /// End, in Unix seconds.
    pub finished_at: Option<u64>,
    /// Why the loop stopped.
    pub stop_reason: Option<StopReason>,
    /// Current incumbent label.
    pub incumbent: String,
    /// Current incumbent's deciding score.
    pub incumbent_score: Option<f64>,
    /// Epochs started so far.
    pub epochs: u32,
    /// Model spend so far, in US dollars.
    pub spend_usd: f64,
    /// Error message for a failed run, scrubbed of key values.
    pub error: Option<String>,
}
