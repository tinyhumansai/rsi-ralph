//! The `rsi.json` wire types. Field names are the stored form.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tinyrsi_score::MetricDirection;

/// The only accepted `$schema`.
pub const SCHEMA: &str = "https://tinyhumans.ai/schemas/rsi/v1";

/// The only accepted `version`.
pub const VERSION: u32 = 1;

/// The model seats, judges and the retreat use when `models` names none.
pub const DEFAULT_MODEL: &str = "z-ai/glm-5.3-flash";

/// The environment variable holding the `OpenRouter` key when `keys` names none.
pub const DEFAULT_OPENROUTER_ENV: &str = "OPENROUTER_API_KEY";

/// A whole `rsi.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Must equal [`SCHEMA`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// Must equal [`VERSION`].
    pub version: u32,
    /// Loop name; names the run-store folder.
    pub name: String,
    /// What is improved.
    pub subject: SubjectSpec,
    /// Markdown prompt template, relative to the loop folder.
    pub brief: String,
    /// The hive that writes candidates.
    pub hive: HiveSpec,
    /// Model choices.
    #[serde(default)]
    pub models: Models,
    /// Temperature and retreat settings.
    #[serde(default)]
    pub creativity: Creativity,
    /// Capability to environment-variable name.
    #[serde(default)]
    pub keys: BTreeMap<String, KeySpec>,
    /// Score functions and the decision rule.
    pub score: ScoreSpec,
    /// Extra gate commands, in order.
    #[serde(default)]
    pub gate: Vec<GateStep>,
    /// Spend limits.
    #[serde(default)]
    pub budget: Budget,
    /// Maximum epochs.
    #[serde(default = "default_epochs")]
    pub epochs: u32,
    /// Epochs without promotion before stopping.
    #[serde(default = "default_patience")]
    pub patience: u32,
    /// Extra environment for every step.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

fn default_epochs() -> u32 {
    3
}

fn default_patience() -> u32 {
    2
}

/// How candidates are materialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    /// A copy of a directory per candidate.
    Dir,
    /// A git worktree per candidate; promotion fast-forwards a branch.
    Git,
}

/// The `subject` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectSpec {
    /// `dir` or `git`.
    pub kind: SubjectKind,
    /// Subject directory, relative to the loop folder (or absolute for `git`).
    pub path: String,
    /// Branch holding the incumbent; required for `git`.
    #[serde(default)]
    pub branch: Option<String>,
    /// Path prefixes the hive may change.
    pub editable: Vec<String>,
    /// Path prefixes the hive must not change; wins over `editable`.
    #[serde(default)]
    pub frozen: Vec<String>,
    /// Reject candidates that change no test file.
    #[serde(default)]
    pub require_test_change: bool,
}

/// The `hive` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HiveSpec {
    /// Package folder, relative to the loop folder.
    pub package: String,
    /// Seat that receives the brief.
    pub lead: String,
    /// Wall-clock limit for one epoch's hive, in seconds.
    #[serde(default = "default_wall_secs")]
    pub wall_secs: u64,
    /// Limit for one seat turn, in seconds.
    #[serde(default = "default_turn_secs")]
    pub turn_secs: u64,
}

fn default_wall_secs() -> u64 {
    3600
}

fn default_turn_secs() -> u64 {
    1200
}

/// The `models` section; every field falls back to [`DEFAULT_MODEL`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Models {
    /// Seats without their own `model`.
    #[serde(default)]
    pub default: Option<String>,
    /// Judge scorers.
    #[serde(default)]
    pub judge: Option<String>,
    /// The creative retreat.
    #[serde(default)]
    pub retreat: Option<String>,
}

/// The `creativity` section.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Creativity {
    /// Default seat temperature, 0 to 2.
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Creative retreat before each epoch's hive.
    #[serde(default)]
    pub retreat: Option<Retreat>,
}

/// A tinybackrooms creative retreat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retreat {
    /// Off switch that keeps the settings.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Independent conversations per epoch: breadth.
    #[serde(default = "three")]
    pub variations: u32,
    /// Turns per conversation: depth.
    #[serde(default = "six")]
    pub turns: u32,
    /// Ideas passed into the brief.
    #[serde(default = "five")]
    pub ideas: u32,
    /// `backrooms` (two agents), `brainstorm` (one) or `critic_pair`.
    #[serde(default)]
    pub mode: Option<String>,
    /// Conversation framing: `cli`, `fugue` or `student`.
    #[serde(default)]
    pub template: Option<String>,
    /// Model, overriding `models.retreat`.
    #[serde(default)]
    pub model: Option<String>,
    /// Sampling temperature for the conversation turns, 0 to 2.
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Steer away from ideas earlier runs already tried.
    #[serde(default = "yes")]
    pub memory: bool,
}

fn yes() -> bool {
    true
}

fn three() -> u32 {
    3
}

fn six() -> u32 {
    6
}

fn five() -> u32 {
    5
}

/// One `keys` entry: an environment variable name, or a tool binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeySpec {
    /// The name of the variable holding the key.
    Env(String),
    /// A tool capability for some or all seats.
    Tool(ToolKey),
}

/// A tool capability such as web search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolKey {
    /// Provider name, such as `gemini`.
    pub provider: String,
    /// The variable holding the key.
    pub env: String,
    /// Seats that get the tool; all seats when absent.
    #[serde(default)]
    pub seats: Option<Vec<String>>,
}

/// The `score` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreSpec {
    /// Score functions; at least one.
    pub scorers: Vec<ScorerSpec>,
    /// Which scorer decides, and how.
    pub decide: DecideSpec,
}

/// One score function: exactly one of `run` and `judge`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScorerSpec {
    /// Unique id.
    pub id: String,
    /// Command run under the step contract.
    #[serde(default)]
    pub run: Option<String>,
    /// Markdown rubric graded by a model.
    #[serde(default)]
    pub judge: Option<String>,
    /// Weight in the combined score.
    #[serde(default = "one")]
    pub weight: f64,
    /// Timeout for `run`, in seconds.
    #[serde(default = "default_step_secs")]
    pub timeout_secs: u64,
    /// Independent judge raters.
    #[serde(default = "three")]
    pub raters: u32,
}

fn one() -> f64 {
    1.0
}

fn default_step_secs() -> u64 {
    600
}

/// The statistical test that decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecideTest {
    /// Paired per-seed comparison with `tinyrsi_score::decide`.
    Seeds,
    /// Exact sign test on per-item 0/1 scores.
    Sign,
}

/// The `score.decide` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecideSpec {
    /// Id of the deciding `run` scorer.
    pub on: String,
    /// `seeds` or `sign`.
    pub test: DecideTest,
    /// Seeds for `seeds`; at least two distinct.
    #[serde(default)]
    pub seeds: Vec<u64>,
    /// Item split for `sign`.
    #[serde(default)]
    pub split: Option<String>,
    /// Significance level for `sign`.
    #[serde(default = "default_alpha")]
    pub alpha: f64,
    /// Which way is better for `seeds`.
    #[serde(default = "maximize")]
    pub direction: MetricDirection,
    /// Required improvement for `seeds`.
    #[serde(default)]
    pub margin: f64,
    /// Score at which the goal is reached.
    #[serde(default)]
    pub target: Option<f64>,
}

fn default_alpha() -> f64 {
    0.10
}

fn maximize() -> MetricDirection {
    MetricDirection::Maximize
}

/// Where a gate command runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepCwd {
    /// The loop folder.
    #[default]
    Loop,
    /// The candidate checkout.
    Candidate,
}

/// One gate command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateStep {
    /// Command, split on whitespace.
    pub run: String,
    /// Working directory.
    #[serde(default)]
    pub cwd: StepCwd,
    /// Timeout in seconds.
    #[serde(default = "default_step_secs")]
    pub timeout_secs: u64,
}

/// The `budget` section, in US dollars.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// Cap for this run.
    #[serde(default)]
    pub run_usd: Option<f64>,
    /// Cap across every run of every loop.
    #[serde(default)]
    pub total_usd: Option<f64>,
    /// Spend that must remain before a candidate is scored.
    #[serde(default)]
    pub score_reserve_usd: f64,
    /// Price ceilings for models beyond the gateway's built-in ones.
    #[serde(default)]
    pub prices: BTreeMap<String, ModelPrice>,
}

/// A model's price ceiling and the one provider it may be routed to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPrice {
    /// Dollars per million prompt tokens.
    pub prompt_per_mtok: f64,
    /// Dollars per million completion tokens.
    pub completion_per_mtok: f64,
    /// Upstream provider, such as `openai`.
    pub provider: String,
}

/// A validated loop folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Loop {
    /// The parsed manifest.
    pub manifest: Manifest,
    /// The brief template text.
    pub brief: String,
    /// Hive package files, relative to the package folder.
    pub hive: BTreeMap<String, String>,
    /// Judge rubrics by scorer id.
    pub rubrics: BTreeMap<String, String>,
}
