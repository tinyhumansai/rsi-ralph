//! Read-only coordinator adapter and deterministic redacted failure dossiers.
mod types;
use crate::blueprint::NodeStatus;
use crate::{Error, Result};
use std::collections::{BTreeMap, BTreeSet};
use tinyhivemind_hives::{Coordinator, EpisodePhase};
pub use types::{
    Dossier, EpisodeObservation, MemoryHealth, Observation, ObservedEpisodePhase, SeatUsage,
    TranscriptReference, TurnUsage, VerifierOutcome,
};

/// Host transcript cursor; reads never schedule agents or mutate coordinator state.
#[derive(Clone, Debug, Default)]
pub struct Observer {
    cursor: Option<u64>,
}
impl Observer {
    /// Starts before the first retained transcript row.
    #[must_use]
    pub const fn new() -> Self {
        Self { cursor: None }
    }
    /// Sequence of the last successfully observed row.
    #[must_use]
    pub const fn cursor(&self) -> Option<u64> {
        self.cursor
    }
    /// Reads incremental transcript references and current episode statuses.
    ///
    /// The two coordinator reads are not an atomic cross-API snapshot. The
    /// cursor advances only after both succeed; failed reads can be retried.
    /// Private message and failure text are discarded before returning.
    ///
    /// # Errors
    /// Returns [`Error::ObservationFailed`] if either coordinator read fails.
    pub fn snapshot(&mut self, coordinator: &Coordinator) -> Result<Observation> {
        self.read_snapshot(
            || {
                coordinator
                    .read_transcript(self.cursor)
                    .map_err(|_| Error::ObservationFailed)
            },
            || coordinator.episodes().map_err(|_| Error::ObservationFailed),
        )
        .map(|(observation, cursor)| {
            self.cursor = cursor;
            observation
        })
    }
    fn read_snapshot(
        &self,
        transcript: impl FnOnce() -> Result<Vec<tinyhivemind_hives::Message>>,
        episodes: impl FnOnce() -> Result<Vec<tinyhivemind_hives::EpisodeStatus>>,
    ) -> Result<(Observation, Option<u64>)> {
        let rows = transcript()?;
        let episodes = episodes()?;
        let cursor = rows.iter().map(|row| row.sequence).max().or(self.cursor);
        let mut transcript: Vec<_> = rows
            .into_iter()
            .map(|row| TranscriptReference {
                sequence: row.sequence,
                seat_id: row.sender,
                episode_id: row.episode_id,
            })
            .collect();
        transcript.sort_by_key(|row| row.sequence);
        let mut episodes: Vec<_> = episodes
            .into_iter()
            .map(|episode| EpisodeObservation {
                episode_id: episode.episode_id,
                opened_at: episode.opened_at,
                phase: match episode.phase {
                    EpisodePhase::Open => ObservedEpisodePhase::Open,
                    EpisodePhase::AwaitingRelease => ObservedEpisodePhase::AwaitingRelease,
                    EpisodePhase::Settled => ObservedEpisodePhase::Settled,
                    EpisodePhase::Failed(_) => ObservedEpisodePhase::Failed,
                },
            })
            .collect();
        episodes.sort_by(|a, b| a.episode_id.cmp(&b.episode_id));
        Ok((
            Observation {
                transcript,
                episodes,
            },
            cursor,
        ))
    }
}
impl Dossier {
    /// Aggregates public references and explicit host usage/outcomes.
    ///
    /// Ordering is independent of input order; contradictory node outcomes
    /// retain both failure and open evidence. Token counters saturate instead
    /// of wrapping. This function neither runs verifiers nor writes memory.
    #[must_use]
    pub fn build(
        epoch: u64,
        observation: &Observation,
        turns: &[TurnUsage],
        outcomes: &[VerifierOutcome],
        memory_health: MemoryHealth,
    ) -> Self {
        let mut seat_usage: BTreeMap<String, SeatUsage> = BTreeMap::new();
        for turn in turns {
            let usage = seat_usage.entry(turn.seat_id.clone()).or_default();
            usage.turns = usage.turns.saturating_add(1);
            usage.input_tokens = usage.input_tokens.saturating_add(turn.input_tokens);
            usage.output_tokens = usage.output_tokens.saturating_add(turn.output_tokens);
            usage.wasted_turns = usage.wasted_turns.saturating_add(u64::from(turn.wasted));
        }
        Self {
            epoch,
            transcript_sequences: observation
                .transcript
                .iter()
                .map(|row| row.sequence)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            failed_episodes: observation
                .episodes
                .iter()
                .filter(|episode| episode.phase == ObservedEpisodePhase::Failed)
                .map(|episode| episode.episode_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            open_episodes: observation
                .episodes
                .iter()
                .filter(|episode| {
                    matches!(
                        episode.phase,
                        ObservedEpisodePhase::Open | ObservedEpisodePhase::AwaitingRelease
                    )
                })
                .map(|episode| episode.episode_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            failed_nodes: outcomes
                .iter()
                .filter(|outcome| outcome.status == NodeStatus::Failed)
                .map(|outcome| outcome.node_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            open_nodes: outcomes
                .iter()
                .filter(|outcome| matches!(outcome.status, NodeStatus::Open | NodeStatus::Sorry))
                .map(|outcome| outcome.node_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            wasted_turns: seat_usage.values().fold(0_u64, |total, usage| {
                total.saturating_add(usage.wasted_turns)
            }),
            seat_usage,
            memory_health,
        }
    }
}
#[cfg(test)]
mod test;
