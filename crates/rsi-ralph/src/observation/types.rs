//! Public observations contain identifiers and counters, never message bodies.
use crate::NodeStatus;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// A transcript row reference with its private text omitted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranscriptReference {
    /// Durable transcript sequence.
    pub sequence: u64,
    /// Originating seat identity.
    pub seat_id: String,
    /// Owning episode, when present.
    pub episode_id: Option<String>,
}
/// Redacted episode lifecycle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ObservedEpisodePhase {
    /// Running or queued.
    Open,
    /// Waiting for host release.
    AwaitingRelease,
    /// Finished normally.
    Settled,
    /// Failed; private failure text is omitted.
    Failed,
}
/// Episode identity and lifecycle without task or failure text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EpisodeObservation {
    /// Durable episode identity.
    pub episode_id: String,
    /// Opening transcript sequence.
    pub opened_at: u64,
    /// Redacted lifecycle.
    pub phase: ObservedEpisodePhase,
}
/// Incremental transcript references and current retained episode statuses.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Observation {
    /// Transcript rows after the observer's previous cursor.
    pub transcript: Vec<TranscriptReference>,
    /// All retained episodes, ordered by identity.
    pub episodes: Vec<EpisodeObservation>,
}
/// Host-measured token usage for one turn.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TurnUsage {
    /// Seat charged for the turn.
    pub seat_id: String,
    /// Input tokens measured by the host.
    pub input_tokens: u64,
    /// Output tokens measured by the host.
    pub output_tokens: u64,
    /// Whether the host classified this turn as wasted.
    pub wasted: bool,
}
/// Aggregate host-measured seat cost.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SeatUsage {
    /// Total turns.
    pub turns: u64,
    /// Total input tokens, saturating on overflow.
    pub input_tokens: u64,
    /// Total output tokens, saturating on overflow.
    pub output_tokens: u64,
    /// Turns explicitly classified as wasted by the host.
    pub wasted_turns: u64,
}
/// An external host's node outcome; the dossier does not run a verifier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerifierOutcome {
    /// Blueprint identity.
    pub node_id: String,
    /// Host-observed node state.
    pub status: NodeStatus,
}
/// Host-supplied memory health counters; building a dossier writes no memory.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct MemoryHealth {
    /// Retrieval failures.
    pub retrieval_failures: u64,
    /// Memory write failures.
    pub write_failures: u64,
    /// Stale or conflicting memories encountered.
    pub stale_entries: u64,
    /// Attempts that repeat a previously failed approach.
    pub repeated_failed_attempts: u64,
    /// Retrieved memories never used by the host.
    pub unused_recalls: u64,
    /// Retrieved memories clipped by context limits.
    pub clipped_recalls: u64,
    /// Mutually contradictory memories encountered.
    pub contradictory_entries: u64,
}
/// Deterministic public failure evidence for one improvement epoch.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Dossier {
    /// Host-selected epoch identity.
    pub epoch: u64,
    /// Sorted unique durable transcript references.
    pub transcript_sequences: Vec<u64>,
    /// Episode identities currently failed.
    pub failed_episodes: Vec<String>,
    /// Episode identities running or awaiting release.
    pub open_episodes: Vec<String>,
    /// Node identities with any failed host outcome.
    pub failed_nodes: Vec<String>,
    /// Node identities with any open or sorry host outcome.
    pub open_nodes: Vec<String>,
    /// Cost by seat, ordered by seat identity.
    pub seat_usage: BTreeMap<String, SeatUsage>,
    /// Total explicitly wasted turns, saturating on overflow.
    pub wasted_turns: u64,
    /// Explicit host memory health snapshot.
    pub memory_health: MemoryHealth,
}
