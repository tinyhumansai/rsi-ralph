//! The rows the meter writes and the caller tags they carry.

use serde::Serialize;

/// Who made a call, from `x-tinyrsi-*` headers or a `/__tag/` path prefix.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Tags {
    /// Loop phase, such as `propose`, `score`, `judge` or `evolve`.
    pub(crate) phase: String,
    /// The seat (agent or role) within the phase, if any.
    pub(crate) seat: String,
    /// The candidate being built or measured, if any.
    pub(crate) candidate: String,
}

/// One metered call. Its serialized form is one line of `meter.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct MeterRow {
    /// Milliseconds since the Unix epoch.
    pub(crate) at_ms: u128,
    /// Caller tags, flattened into the row.
    #[serde(flatten)]
    pub(crate) tags: Tags,
    /// Model id.
    pub(crate) model: String,
    /// `chat/completions` or `embeddings`.
    pub(crate) endpoint: String,
    /// HTTP status returned to the caller.
    pub(crate) status: u16,
    /// Whether the response streamed.
    pub(crate) stream: bool,
    /// Dollars reserved.
    pub(crate) reserved_usd: f64,
    /// Dollars charged on settlement, if known.
    pub(crate) cost_usd: Option<f64>,
    /// Prompt tokens.
    pub(crate) prompt_tokens: u64,
    /// Completion tokens.
    pub(crate) completion_tokens: u64,
    /// Prompt tokens the provider served from its prompt cache, as it
    /// reported them. `None` (stored as `null`) when it did not report any
    /// count, which is unknown rather than zero hits.
    pub(crate) cached_tokens: Option<u64>,
    /// Completion tokens spent reasoning, as the provider reported them.
    /// `None` (stored as `null`) when it did not say.
    pub(crate) reasoning_tokens: Option<u64>,
    /// Wall-clock duration.
    pub(crate) wall_ms: u128,
    /// Time to first token: from forwarding the call to the first stream
    /// event carrying generated output. `None` for a buffered response or a
    /// stream that produced nothing.
    pub(crate) ttft_ms: Option<u128>,
    /// Refusal or failure reason.
    pub(crate) error: Option<String>,
}
