//! The upgrade log's records. Field names are the stored form: every line of
//! an `upgrades.jsonl` is one [`Entry`], tagged by `kind`.

use serde::{Deserialize, Serialize};

/// One line of an `upgrades.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    /// A change to the hive package or the loop config, and why.
    Upgrade(Upgrade),
    /// What a later run measured against an upgrade's predictions.
    Outcome(Outcome),
    /// What a run found changed since the loop's previous run, written by
    /// the run itself when it starts.
    Applied(Applied),
}

/// A change to the hive package or the loop config: what was seen, what was
/// changed, what it should do, and who said so.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Upgrade {
    /// Unique id within the loop, such as `u-20261025-110412`.
    pub id: String,
    /// When it was recorded, in Unix seconds.
    pub recorded_at: u64,
    /// One-line name, such as `Keep every seat working`.
    pub title: String,
    /// Everyone who took part, each with their own rationale. At least one.
    pub contributors: Vec<Contributor>,
    /// The run and epoch it takes effect from. A run left unset means the
    /// next run of the loop; the run that picks it up says so in its
    /// [`Applied`] entry.
    #[serde(default, skip_serializing_if = "AppliesFrom::is_unset")]
    pub applies_from: AppliesFrom,
    /// What the change was based on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
    /// The root cause the evidence points to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnosis: Option<String>,
    /// What changed.
    pub change: Change,
    /// What the change should do, each a measurable prediction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected: Vec<Prediction>,
}

/// One party to an upgrade and the reasoning they gave, in their own words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contributor {
    /// Who.
    pub author: Author,
    /// What they did.
    pub role: Role,
    /// Their reason, verbatim: the text an agent gave, never a summary
    /// written afterwards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

/// Who made or took part in a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Author {
    /// The human running the loop.
    Operator {
        /// Their name or handle.
        name: String,
    },
    /// An AI assistant working with the operator, such as a coding agent.
    Assistant {
        /// The agent, such as `claude-code`.
        name: String,
        /// The model behind it, such as `claude-opus-5-5`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    },
    /// A seat of the hive, the lead included.
    Hive {
        /// The seat id, such as `lead`.
        seat: String,
    },
    /// The loop's own machinery, such as the run-start diff.
    Loop {
        /// The mechanism, such as `run_start_diff`.
        mechanism: String,
    },
}

/// What a contributor did for an upgrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Wrote the whole change: the default for a single contributor.
    Authored,
    /// Found the root cause.
    Diagnosed,
    /// Suggested the change.
    Proposed,
    /// Agreed to the change before it was applied.
    Approved,
    /// Made the edit.
    Applied,
    /// Noticed a change nobody recorded.
    Detected,
}

/// Where an upgrade takes effect.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliesFrom {
    /// Run id; unset means the next run of the loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Epoch within that run; unset means from its start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epoch: Option<u32>,
}

impl AppliesFrom {
    /// Neither a run nor an epoch is set.
    #[must_use]
    pub fn is_unset(&self) -> bool {
        self.run.is_none() && self.epoch.is_none()
    }
}

/// One thing an upgrade was based on: a measured number, a claim, or an
/// observation in words, with where it came from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Metric name, such as `seats_busy_mean`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    /// The metric's measured value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The run it was measured in, as `<loop>/<run-id>` or a run id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// A claim id it rests on, such as a ledger record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<String>,
    /// The observation in words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The edit itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// What changed, in words.
    pub summary: String,
    /// The files touched, relative to the loop folder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileChange>,
    /// A diff or diff summary, when one was taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// One file an upgrade touched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    /// Path relative to the loop folder, such as `hive/seats/lead.md`; a
    /// path ending in `/` names every file below it.
    pub path: String,
    /// Keys touched inside it, such as `max_turns`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,
    /// SHA-256 of the file before, hex; unset when it did not exist or was
    /// not known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// SHA-256 of the file after, hex; unset when it was removed or not
    /// known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// A measurable prediction: `metric op value`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    /// Metric name, as the next run's evidence will name it.
    pub metric: String,
    /// How the measured value should compare.
    pub op: Comparison,
    /// The threshold.
    pub value: f64,
    /// Context, such as the value before the change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A comparison in a [`Prediction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    /// Strictly greater.
    Gt,
    /// Greater or equal.
    Ge,
    /// Strictly less.
    Lt,
    /// Less or equal.
    Le,
    /// Equal, to within 1e-9.
    Eq,
}

/// What a later run measured against one upgrade's predictions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    /// The upgrade it judges.
    pub upgrade: String,
    /// When it was recorded, in Unix seconds.
    pub recorded_at: u64,
    /// The run that measured it.
    pub run: String,
    /// Whether the predictions held.
    pub verdict: Verdict,
    /// The measured values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measured: Vec<Evidence>,
    /// Who judged it, with their reasoning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributors: Vec<Contributor>,
}

/// Whether an upgrade's predictions held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Every prediction held.
    Confirmed,
    /// None held.
    Refuted,
    /// Some held.
    Mixed,
}

/// What a run found changed since the loop's previous run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Applied {
    /// The run that started.
    pub run: String,
    /// When it started, in Unix seconds.
    pub recorded_at: u64,
    /// The previous run it was compared with; unset for a loop's first run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_run: Option<String>,
    /// Upgrades this run is the first to carry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upgrades: Vec<String>,
    /// Every file that differs from the previous run's, with hashes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileChange>,
    /// The changed paths no upgrade names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unexplained: Vec<String>,
}
