//! A swarm run's inputs and result.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::event::{REDACTED, StopReason};
use crate::tools::ToolKey;

/// Maps a seat id (or [`crate::RUNTIME_SEAT`] for the runtime's own calls)
/// to the OpenAI-compatible base URL its model calls go to, e.g. a spend
/// gateway's per-seat tagged URL ending in `/v1`.
pub type ModelUrl = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// Everything one swarm run needs.
///
/// Git work is the caller's: create the candidate checkouts before the run,
/// and commit, diff or discard them after it.
pub struct SwarmRequest {
    /// The hive package, relative path → text; see [`crate::load_package`].
    pub package: BTreeMap<String, String>,
    /// The seat the brief is sent to as a direct message.
    pub lead: String,
    /// The epoch brief. Each candidate's checkout path is appended to it.
    pub brief: String,
    /// Where the run writes `hive.db`, `events.jsonl` and the agents'
    /// `OpenHuman` workspace (`runtime/`, which holds their transcripts).
    /// Use a fresh directory per run.
    pub run_dir: PathBuf,
    /// The incumbent checkout, which non-editing seats read but never write.
    pub incumbent: PathBuf,
    /// One editable checkout per editing seat; see [`crate::editing_seats`].
    pub candidates: BTreeMap<String, PathBuf>,
    /// Each seat's model base URL.
    pub model_base_url: ModelUrl,
    /// The API key sent to every model base URL. A spend gateway that holds
    /// the real key can take any placeholder.
    pub model_api_key: String,
    /// The model of every seat whose package entry names none.
    pub default_model: String,
    /// The temperature of every seat whose `config.temperature` is unset;
    /// `None` keeps the harness default.
    pub temperature: Option<f64>,
    /// Provider keys for seat tools, such as web search.
    pub tools: Vec<ToolKey>,
    /// The longest the coordinator runs before the swarm stops it.
    pub wall: Duration,
    /// The longest one seat turn may take.
    pub turn: Duration,
}

impl std::fmt::Debug for SwarmRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwarmRequest")
            .field("package", &self.package.keys().collect::<Vec<_>>())
            .field("lead", &self.lead)
            .field("run_dir", &self.run_dir)
            .field("incumbent", &self.incumbent)
            .field("candidates", &self.candidates)
            .field("model_api_key", &REDACTED)
            .field("default_model", &self.default_model)
            .field("temperature", &self.temperature)
            .field("tools", &self.tools)
            .field("wall", &self.wall)
            .field("turn", &self.turn)
            .finish_non_exhaustive()
    }
}

/// How a swarm run ended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SwarmOutcome {
    /// Whether the coordinator went idle or the wall clock stopped it.
    pub stop: StopReason,
    /// Turns that returned normally (zero when stopped by the wall clock,
    /// which interrupts the drain before it reports).
    pub completed: usize,
    /// Turns that failed.
    pub failed: usize,
    /// Turns parked awaiting release.
    pub parked: usize,
    /// Every seat's turns and spend, including seats that never ran.
    pub seats: BTreeMap<String, SeatUsage>,
}

/// One seat's turns and spend over a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeatUsage {
    /// Turns the seat finished, successfully or not.
    pub turns: u64,
    /// Input tokens across those turns.
    pub input_tokens: u64,
    /// Output tokens across those turns.
    pub output_tokens: u64,
    /// Known cost across those turns, in US dollars.
    pub cost_usd: f64,
    /// Whether every turn's cost was known; when `false`, `cost_usd` is a
    /// lower bound.
    pub cost_complete: bool,
}

impl Default for SeatUsage {
    fn default() -> Self {
        Self {
            turns: 0,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: 0.0,
            cost_complete: true,
        }
    }
}
