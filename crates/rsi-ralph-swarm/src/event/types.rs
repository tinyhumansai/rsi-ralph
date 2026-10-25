//! Swarm event payloads and their stored JSON form.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::Redactor;

/// One thing that happened in a running swarm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SwarmEvent {
    /// Every seat is registered and the brief is about to be sent.
    SwarmStarted {
        /// The package's hive id.
        hive: String,
        /// The seat the brief is sent to.
        lead: String,
        /// The desks created on the coordinator.
        desks: Vec<String>,
        /// Every seat, in manifest order.
        seats: Vec<SeatInfo>,
    },
    /// A message entered the coordinator transcript: the brief, a
    /// seat-to-seat direct message, or a desk post.
    AgentMessage {
        /// The coordinator's monotonic sequence number.
        sequence: u64,
        /// The sending seat, or the host.
        from: String,
        /// The receiving seat, or `hive:<id>` for a desk post.
        to: String,
        /// The message text, clipped.
        body: String,
        /// Private readers of a desk post; empty means every member.
        only_for: Vec<String>,
        /// The conducted episode the message belongs to, if any.
        episode: Option<String>,
    },
    /// A seat started or finished a tool call.
    ToolCall {
        /// The calling seat.
        seat: String,
        /// The tool's name.
        tool: String,
        /// Whether the call started or completed.
        phase: ToolPhase,
        /// A short human label or the clipped arguments, when started.
        detail: Option<String>,
        /// Whether the call succeeded, when completed.
        success: Option<bool>,
        /// The size of the tool's output, when completed.
        output_chars: Option<usize>,
    },
    /// A seat finished a turn.
    AgentTurn {
        /// The seat.
        seat: String,
        /// Input tokens the turn spent, summed over its model calls.
        input_tokens: u64,
        /// Output tokens the turn spent.
        output_tokens: u64,
        /// The turn's cost, when every call's cost was known.
        cost_usd: Option<f64>,
    },
    /// The coordinator went idle or the wall clock ran out.
    SwarmFinished {
        /// Why the swarm stopped.
        stop: StopReason,
        /// Turns that returned normally.
        completed: usize,
        /// Turns that failed.
        failed: usize,
        /// Turns parked awaiting release.
        parked: usize,
    },
}

/// A seat as announced in [`SwarmEvent::SwarmStarted`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatInfo {
    /// The seat id.
    pub id: String,
    /// The role it fills.
    pub role: String,
    /// The model it runs on.
    pub model: String,
    /// The checkout it works in.
    pub action_dir: PathBuf,
    /// Whether that checkout is its own editable candidate.
    pub editing: bool,
    /// The memory namespace its conversations are logged in, when seats
    /// carry memory; shared memory gives every seat the same one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

/// The two ends of a tool call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPhase {
    /// The model asked for the call.
    Started,
    /// The tool returned.
    Completed,
}

/// Why [`crate::run_swarm`] stopped the coordinator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// No seat had work left: the lead stopped sending.
    Idle,
    /// `SwarmRequest::wall` elapsed first.
    WallClock,
}

impl SwarmEvent {
    /// This event with every secret removed from its free text.
    pub(crate) fn scrubbed(self, redactor: &Redactor) -> Self {
        match self {
            Self::AgentMessage {
                sequence,
                from,
                to,
                body,
                only_for,
                episode,
            } => Self::AgentMessage {
                sequence,
                from,
                to,
                body: redactor.scrub(&body),
                only_for,
                episode,
            },
            Self::ToolCall {
                seat,
                tool,
                phase,
                detail,
                success,
                output_chars,
            } => Self::ToolCall {
                seat,
                tool,
                phase,
                detail: detail.map(|detail| redactor.scrub(&detail)),
                success,
                output_chars,
            },
            other => other,
        }
    }
}
