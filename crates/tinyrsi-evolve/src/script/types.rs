//! What the script supervisor reads, what it proposes, and its call limits.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What the supervisor reads: the prompt the loop rendered from its ledger,
/// the files a proposal may rewrite, and how to sample.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptBrief {
    /// The full user prompt: goal, metric history, approaches already tried,
    /// rejections, sources and this slot's directive.
    pub prompt: String,
    /// Workspace-relative paths a proposal may replace.
    pub editable: Vec<PathBuf>,
    /// Sampling temperature for this call, or `None` for the model's default
    /// (`0.0` for `tinyrsi_model::ModelRequest`). A supervisor that cannot
    /// sample, such as a canned replay, ignores it.
    pub temperature: Option<f64>,
}

/// Per-call limits of the supervisor's model requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorLimits {
    /// Whole-call token cap, prompt included.
    pub call_tokens: u64,
    /// Output token cap.
    pub output_tokens: u64,
    /// Provider timeout in milliseconds (at most 300 000).
    pub timeout_ms: u64,
}

impl Default for SupervisorLimits {
    fn default() -> Self {
        Self {
            call_tokens: 200_000,
            output_tokens: 16_384,
            timeout_ms: 180_000,
        }
    }
}

/// One whole file a proposal writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedFile {
    /// Workspace-relative path; must be one of the subject's editable files.
    pub path: String,
    /// Complete new file content.
    pub content: String,
}

/// The supervisor's structured proposal, the schema of its forced tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptProposal {
    /// A short label naming the technique, such as `"tabu search"` or
    /// `"cubic least squares"`; the loop groups attempts by it. The tool
    /// schema requires it, but it defaults to empty (unlabelled) so replies
    /// written before labels existed still decode. See
    /// [`ScriptProposal::approach_label`].
    #[serde(default)]
    pub approach: String,
    /// What is concretely different from an earlier attempt of the same
    /// approach, when this proposal retries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub differs_from_previous: Option<String>,
    /// Why the change should help.
    pub rationale: String,
    /// The checkable expected effect on the metric.
    pub hypothesis: String,
    /// Whole-file replacements; unlisted editable files keep their content.
    pub files: Vec<ProposedFile>,
}
