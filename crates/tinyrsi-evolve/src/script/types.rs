//! What the script supervisor reads, what it proposes, and its call limits.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What the supervisor reads: the prompt the loop rendered from its ledger,
/// and the files a proposal may rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptBrief {
    /// The full user prompt: goal, metric history, rejections, sources.
    pub prompt: String,
    /// Workspace-relative paths a proposal may replace.
    pub editable: Vec<PathBuf>,
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
    /// Why the change should help.
    pub rationale: String,
    /// The checkable expected effect on the metric.
    pub hypothesis: String,
    /// Whole-file replacements; unlisted editable files keep their content.
    pub files: Vec<ProposedFile>,
}
