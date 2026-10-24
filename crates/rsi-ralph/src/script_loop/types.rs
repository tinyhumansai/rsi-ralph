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
    /// Approaches shown under "Approaches already tried", most recently
    /// attempted first; `0` removes the section and its resubmission rule.
    pub approach_context: usize,
    /// One directive per slot of an epoch: slot `k` uses
    /// `slots[k % slots.len()]`. Empty means no directive and the model's
    /// default sampling for every slot.
    pub slots: Vec<SlotDirective>,
    /// Paired acceptance policy; quality units are described in the README.
    pub eval: EvalPolicy,
    /// Accept an `Inconclusive` candidate that is no worse than the incumbent
    /// on any seed and strictly better on at least one, by metric and by
    /// eligible quality (see `rsi_ralph_score::dominates`). Few seeds cannot
    /// resolve an uneven gain with the paired interval alone; for a
    /// deterministic script, per-seed dominance is exact on those seeds.
    pub accept_dominant: bool,
}

impl Default for LoopPolicy {
    /// Five epochs of two proposals, patience two, the default slot
    /// directives ([`SlotDirective::defaults`]), twelve approaches of
    /// context, and dominating candidates accepted.
    fn default() -> Self {
        Self {
            max_epochs: 5,
            proposals_per_epoch: 2,
            patience: 2,
            token_budget: 500_000,
            rejected_context: 3,
            approach_context: 12,
            slots: SlotDirective::defaults(),
            eval: EvalPolicy {
                margin: 0.0,
                ..EvalPolicy::default()
            },
            accept_dominant: true,
        }
    }
}

impl LoopPolicy {
    /// The directive of slot `k`, cycling through [`LoopPolicy::slots`].
    pub(super) fn slot(&self, k: u32) -> Option<&SlotDirective> {
        let index = usize::try_from(k).ok()? % self.slots.len().max(1);
        self.slots.get(index)
    }
}

/// What one proposal slot of an epoch is asked to do, and how to sample it.
///
/// The instruction is the last section of that slot's prompt, so slots of
/// the same epoch ask different questions instead of repeating one.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotDirective {
    /// The slot's task, shown verbatim under "This proposal".
    pub instruction: String,
    /// Sampling temperature in `[0, 2]` requested for the slot, or `None` for
    /// the model's default (`0`).
    pub temperature: Option<f64>,
}

impl SlotDirective {
    /// A directive with the model's default sampling.
    #[must_use]
    pub fn new(instruction: impl Into<String>) -> Self {
        Self {
            instruction: instruction.into(),
            temperature: None,
        }
    }

    /// The same directive sampled at `temperature`; the loop refuses a value
    /// outside `[0, 2]` before the run starts.
    #[must_use]
    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// The default three slots: the best next step, a different family of
    /// approach, and a repair of the most promising rejected approach. None
    /// sets a temperature.
    #[must_use]
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::new(
                "Best next step: propose the single change most likely to improve the metric \
                 from the incumbent now.",
            ),
            Self::new(
                "Explore: propose a different family of approach from every approach already \
                 tried and from the earlier proposals this epoch.",
            ),
            Self::new(
                "Repair: take the most promising rejected approach and fix what its diagnostics \
                 show went wrong; say what changed in `differs_from_previous`. If nothing has \
                 been rejected yet, propose the best next step.",
            ),
        ]
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
    /// The proposal's approach label (`ScriptProposal::approach_label`).
    /// Omitted when absent, as in ledgers written before labels existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approach: Option<String>,
    /// What the proposal said is different from an earlier attempt of its
    /// approach. Omitted when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub differs_from_previous: Option<String>,
    /// Sampling temperature the loop requested for this proposal's slot. A
    /// request, not a guarantee: a canned supervisor ignores it. Omitted when
    /// the slot set none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
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
