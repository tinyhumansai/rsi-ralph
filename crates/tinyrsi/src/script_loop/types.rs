//! Subject, policy, proposal, ledger and report types of the script loop.
use crate::{EvalPolicy, IntervalUnits, MetricDirection, StopReason};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The program the loop runs once per seed inside each candidate directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptCommand {
    /// Program to execute. Prefer an absolute path: the child's `PATH` is only
    /// `/usr/bin:/bin` unless the subject's `env` overrides it.
    pub program: PathBuf,
    /// Arguments, passed verbatim; relative paths resolve in the candidate.
    pub args: Vec<String>,
}

/// The metric a script reports and the goal for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricSpec {
    /// Name in the `METRIC <name>=<value>` stdout line.
    pub name: String,
    /// Which way is better.
    pub direction: MetricDirection,
    /// Stop once the incumbent's mean metric reaches this value.
    pub target: Option<f64>,
}

/// A plain script the loop improves.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptSubject {
    /// Short human-readable name, recorded in the trace.
    pub name: String,
    /// Free-text goal and constraints shown to the supervisor.
    pub brief: String,
    /// Directory copied into every candidate.
    pub workspace: PathBuf,
    /// Workspace-relative UTF-8 files the supervisor may rewrite.
    pub editable: Vec<PathBuf>,
    /// The command measured on each seed.
    pub command: ScriptCommand,
    /// The reported metric and its goal.
    pub metric: MetricSpec,
    /// At least two distinct seeds, passed as `TINYRSI_SEED`.
    pub seeds: Vec<u64>,
    /// Per-seed wall-clock limit in milliseconds.
    pub timeout_ms: u64,
    /// Per-seed limit on combined stdout and stderr bytes.
    pub output_cap: u64,
    /// Extra child environment, applied after `PATH` and `TMPDIR`.
    pub env: Vec<(String, String)>,
}

/// Limits of the outer loop.
#[derive(Debug, Clone, PartialEq)]
pub struct LoopPolicy {
    /// Most improvement epochs after the baseline.
    pub max_epochs: u64,
    /// Proposals requested per epoch.
    pub proposals_per_epoch: u32,
    /// Consecutive epochs without a promotion before stopping as stalled.
    pub patience: u32,
    /// Measured supervisor and scorer tokens after which no new proposal is
    /// requested.
    pub token_budget: u64,
    /// Rejected proposals shown to the supervisor, most recent first.
    pub rejected_context: usize,
    /// Paired acceptance policy; quality units are described in the README.
    pub eval: EvalPolicy,
}

impl Default for LoopPolicy {
    fn default() -> Self {
        Self {
            max_epochs: 5,
            proposals_per_epoch: 2,
            patience: 2,
            token_budget: 500_000,
            rejected_context: 3,
            eval: EvalPolicy {
                margin: 0.0,
                ..EvalPolicy::default()
            },
        }
    }
}

/// Where a run writes its ledger, trace companions and candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLocation {
    /// Run identifier recorded in the trace and ledger.
    pub id: String,
    /// Run directory; `ledger.json`, `candidates/` and `tmp/` live here.
    pub dir: PathBuf,
}

/// What a script-loop scorer sees: one fully measured candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptCandidate {
    /// The candidate's editable files, by workspace-relative path.
    pub sources: std::collections::BTreeMap<PathBuf, String>,
    /// One metric value per seed, in the subject's seed order.
    pub metrics: Vec<f64>,
}

/// One seed of one candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeedRun {
    /// Seed passed as `TINYRSI_SEED`.
    pub seed: u64,
    /// Parsed metric, absent on failure.
    pub metric: Option<f64>,
    /// The failure, rendered from the crate error, absent on success.
    pub error: Option<String>,
    /// Wall-clock milliseconds.
    pub duration_ms: u64,
    /// Up to the last 2 000 bytes of stderr, lossily decoded.
    pub stderr_tail: String,
    /// The script's `DIAG <text>` stdout lines, in order: at most the last 20,
    /// each cut to 300 characters. They reach the supervisor's next prompt, so
    /// a script (and the code under improvement) can report what it did, not
    /// only how well. Absent in ledgers written before diagnostics existed.
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

/// What happened to one ledger candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateDecision {
    /// The starting script.
    Baseline,
    /// Paired comparison accepted it.
    Accept,
    /// It failed a seed or paired comparison rejected it.
    Reject,
    /// The paired interval overlapped the margin.
    Inconclusive,
    /// The supervisor's reply was malformed or escaped the editable set.
    Invalid,
    /// Its content was already evaluated in this run.
    Duplicate,
    /// It was measured, but the scorer failed on it and the run ended.
    Error,
}

/// One ledger row: an evaluated, invalid or duplicate candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerCandidate {
    /// Epoch, 0 for the baseline.
    pub epoch: u64,
    /// Proposal identifier, absent for the baseline.
    pub proposal_id: Option<String>,
    /// Content id, absent for an invalid proposal.
    pub id: Option<String>,
    /// Incumbent the proposal edited, absent for the baseline.
    pub parent: Option<String>,
    /// Supervisor rationale.
    pub rationale: String,
    /// Supervisor hypothesis.
    pub hypothesis: String,
    /// Paths the proposal rewrote.
    pub paths: Vec<String>,
    /// Per-seed outcomes, in subject seed order.
    pub runs: Vec<SeedRun>,
    /// Mean metric over successful seeds.
    pub mean: Option<f64>,
    /// Disposition.
    pub decision: CandidateDecision,
    /// Paired improvement estimate, positive meaning better, in `units`.
    pub estimate: Option<f64>,
    /// Lower bound of the improvement.
    pub lower: Option<f64>,
    /// Upper bound of the improvement.
    pub upper: Option<f64>,
    /// Units of the interval. Omitted from the JSON when they are metric
    /// units, so ledgers written by the default scorer are unchanged.
    #[serde(default, skip_serializing_if = "IntervalUnits::is_metric")]
    pub units: IntervalUnits,
    /// True once it became the incumbent.
    pub promoted: bool,
    /// Why an invalid proposal was refused, or the scorer's error.
    pub note: Option<String>,
}

/// One link of the incumbent chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncumbentEntry {
    /// Epoch the incumbent was promoted in, 0 for the baseline.
    pub epoch: u64,
    /// Content id.
    pub id: String,
    /// Mean metric.
    pub metric: f64,
}

/// The durable `ledger.json` snapshot of a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptLedger {
    /// Run identifier.
    pub run_id: String,
    /// Subject name.
    pub subject: String,
    /// Metric and goal.
    pub metric: MetricSpec,
    /// Seeds every candidate runs on.
    pub seeds: Vec<u64>,
    /// Measured supervisor and scorer tokens.
    pub tokens: u64,
    /// Why the run ended, absent while it is running.
    pub stop: Option<StopReason>,
    /// Incumbent chain, baseline first.
    pub incumbents: Vec<IncumbentEntry>,
    /// Every candidate, in evaluation order.
    pub candidates: Vec<LedgerCandidate>,
}

/// Summary returned by a finished run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoopReport {
    /// Run identifier.
    pub run_id: String,
    /// Why the run ended.
    pub reason: StopReason,
    /// Baseline content id.
    pub baseline: String,
    /// Baseline mean metric.
    pub baseline_metric: f64,
    /// Final incumbent content id.
    pub incumbent: String,
    /// Final incumbent mean metric.
    pub incumbent_metric: f64,
    /// Improvement epochs started.
    pub epochs: u64,
    /// Candidates promoted to incumbent.
    pub promotions: u32,
    /// Candidates evaluated and not accepted, plus invalid and duplicate ones.
    pub rejected: u32,
    /// Measured supervisor and scorer tokens.
    pub tokens: u64,
    /// Path of `ledger.json`.
    pub ledger: PathBuf,
}
