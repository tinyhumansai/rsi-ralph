//! The step contract's inputs and the `result.json` a scorer writes.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::Duration,
};

use serde::{Deserialize, Serialize};

/// Everything a step command can see about where it runs.
#[derive(Debug, Clone)]
pub struct StepContext {
    /// The loop folder (`RSI_LOOP_DIR`).
    pub loop_dir: PathBuf,
    /// The run folder (`RSI_RUN_DIR`).
    pub run_dir: PathBuf,
    /// The incumbent checkout (`RSI_INCUMBENT`).
    pub incumbent: PathBuf,
    /// Epoch number (`RSI_EPOCH`).
    pub epoch: u32,
    /// Base URL for model calls; the step id is appended as a tag.
    pub model_url: Option<String>,
    /// The manifest's `env` map.
    pub env: BTreeMap<String, String>,
    /// `PATH` and `HOME` passed through from the caller.
    pub inherited: Vec<(String, String)>,
}

/// One invocation of a step command.
#[derive(Debug, Clone)]
pub struct StepRun {
    /// Step id, such as `score.colors` or `gate.1`.
    pub id: String,
    /// Command line, split on whitespace.
    pub command: String,
    /// Working directory.
    pub cwd: PathBuf,
    /// The checkout being judged (`RSI_CANDIDATE`).
    pub candidate: PathBuf,
    /// `incumbent` or the candidate id (`RSI_LABEL`).
    pub label: String,
    /// Seed (`RSI_SEED`).
    pub seed: u64,
    /// Folder for this invocation: `stdout`, `stderr` and `out/`.
    pub dir: PathBuf,
    /// Wall-clock limit.
    pub timeout: Duration,
}

/// How a step ended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepOutcome {
    /// Step id.
    pub id: String,
    /// True for a zero exit status.
    pub ok: bool,
    /// Why it failed: the last stderr line, a timeout, or an output overflow.
    pub reason: Option<String>,
    /// Wall-clock milliseconds.
    pub duration_ms: u64,
    /// Full stdout, lossily decoded.
    #[serde(skip)]
    pub stdout: String,
}

/// One split's per-item scores.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Split {
    /// Item id to score, usually 0 or 1.
    #[serde(default)]
    pub items: BTreeMap<String, f64>,
}

/// One failure shown to the hive in the next brief.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    /// Item id.
    pub id: String,
    /// Optional category, used to spread the sample across kinds.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    /// Markdown shown to the hive.
    pub text: String,
}

/// What a scorer reports: `result.json`, or a `METRIC` line.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScoreResult {
    /// The headline score.
    pub score: f64,
    /// Per-item scores by split.
    #[serde(default)]
    pub splits: BTreeMap<String, Split>,
    /// Failures to show the hive.
    #[serde(default)]
    pub failures: Vec<Failure>,
    /// Extra named numbers.
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
}
