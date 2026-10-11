//! Wire types of the loop trace; field order is part of the contract.
use serde::{Deserialize, Serialize};

/// Whether a lower or a higher metric value is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricDirection {
    /// Lower values are better, as for a loss or an error.
    Minimize,
    /// Higher values are better, as for an accuracy or a score.
    Maximize,
}

/// What the run is trying to achieve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunGoal {
    /// Metric name.
    pub metric: String,
    /// Which way is better.
    pub direction: MetricDirection,
    /// Value that ends the run once the incumbent reaches it, if any.
    pub target: Option<f64>,
}

/// Paired scoring disposition of one candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreDecision {
    /// The candidate beat the incumbent beyond the practical margin.
    Accept,
    /// The candidate failed, regressed, or could not beat the margin.
    Reject,
    /// The interval overlaps the margin.
    Inconclusive,
}

/// Why a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The incumbent met the target.
    GoalReached,
    /// The epoch limit was reached.
    MaxEpochs,
    /// Too many consecutive epochs passed without a promotion.
    Stalled,
    /// The supervisor token budget was exhausted.
    Budget,
    /// The run failed.
    Error,
}

/// One loop event; serialized internally tagged by `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoopEvent {
    /// The run began.
    RunStarted {
        /// Run identifier.
        run_id: String,
        /// Human-readable subject name.
        subject: String,
        /// Goal of the run.
        goal: RunGoal,
    },
    /// An epoch began from the named incumbent.
    EpochStarted {
        /// Epoch number; the baseline is epoch 0.
        epoch: u64,
        /// Incumbent content id.
        incumbent: String,
    },
    /// The supervisor's context was assembled.
    Observed {
        /// Epoch number.
        epoch: u64,
        /// Incumbent content id.
        incumbent: String,
        /// Incumbent mean metric.
        incumbent_metric: Option<f64>,
        /// Number of metric-history entries shown.
        history: u32,
        /// Number of rejected proposals so far.
        rejected: u32,
    },
    /// The supervisor returned one proposal.
    Ideated {
        /// Epoch number.
        epoch: u64,
        /// Proposal identifier, unique within the run.
        proposal_id: String,
        /// Incumbent the proposal edits.
        parent: String,
        /// Why the supervisor expects improvement.
        rationale: String,
        /// The checkable claim.
        hypothesis: String,
    },
    /// One seed of one candidate ran.
    Executed {
        /// Epoch number.
        epoch: u64,
        /// Candidate content id.
        candidate: String,
        /// Seed passed to the script.
        seed: u64,
        /// Parsed metric, absent when the seed failed.
        metric: Option<f64>,
        /// True when the seed produced a valid metric.
        ok: bool,
        /// Wall-clock milliseconds of the run.
        duration_ms: u64,
    },
    /// Every seed of one candidate ran.
    Measured {
        /// Epoch number.
        epoch: u64,
        /// Candidate content id.
        candidate: String,
        /// Mean metric over successful seeds, absent when none succeeded.
        mean: Option<f64>,
        /// Failed seeds.
        failures: u32,
        /// Seeds run.
        seeds: u32,
    },
    /// A candidate was compared with the incumbent.
    Scored {
        /// Epoch number.
        epoch: u64,
        /// Candidate content id.
        candidate: String,
        /// Disposition.
        decision: ScoreDecision,
        /// Mean paired improvement, positive meaning better.
        estimate: f64,
        /// Lower interval bound of the improvement.
        lower: f64,
        /// Upper interval bound of the improvement.
        upper: f64,
    },
    /// A candidate replaced the incumbent.
    Improved {
        /// Epoch number.
        epoch: u64,
        /// Previous incumbent content id.
        from: String,
        /// New incumbent content id.
        to: String,
        /// New incumbent mean metric.
        metric: f64,
    },
    /// An epoch ended.
    EpochFinished {
        /// Epoch number.
        epoch: u64,
        /// Incumbent content id after the epoch.
        incumbent: String,
        /// Incumbent mean metric after the epoch.
        incumbent_metric: Option<f64>,
    },
    /// The run ended.
    RunFinished {
        /// Why the run ended.
        reason: StopReason,
        /// Final incumbent content id.
        incumbent: String,
        /// Final incumbent mean metric.
        incumbent_metric: Option<f64>,
    },
}

/// One line of `events.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceRecord {
    /// Line number, from 0 without gaps.
    pub seq: u64,
    /// Milliseconds since the run started, from the injected clock.
    pub elapsed_ms: u64,
    /// The event.
    pub event: LoopEvent,
}
