//! Restart-safe measured turn callbacks and atomic redacted epoch receipts.
mod storage;
mod types;
mod validation;
use crate::{
    Dossier, Error, MemoryHealth, NodeStatus, ObservedEpisodePhase, Observer, Result, TurnUsage,
    VerificationEvidence, VerifierOutcome,
};
use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
};
use tinyhivemind_hives::Coordinator;
use types::State;
pub use types::{EvidenceEpoch, RecordedTurn, RecordedTurnStatus};
use validation::{validate, validate_checker};

/// Exclusive host journal for measured callbacks and exactly-once epoch receipts.
///
/// Stop admission and join runners before checkpointing or interrupting turns.
/// This store contains redacted identities/counters, never model or oracle text.
#[derive(Debug)]
pub struct EvidenceJournal {
    store: Mutex<storage::Store>,
}
impl EvidenceJournal {
    /// Open retained evidence, validating replay and single-writer ownership.
    /// # Errors
    /// Returns storage, locking or corrupt-evidence errors.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            store: Mutex::new(storage::Store::open(path.as_ref())?),
        })
    }
    /// Acquires the collector; poisoning prevents further evidence mutation.
    fn lock(&self) -> Result<MutexGuard<'_, storage::Store>> {
        self.store.lock().map_err(|_| Error::HostFailed)
    }
    /// Persist an invocation identity before calling a provider.
    /// # Errors
    /// Rejects empty identities, exhausted IDs or persistence failure.
    pub fn begin_turn(&self, seat_id: &str, episode_id: &str) -> Result<u64> {
        if seat_id.trim().is_empty() || episode_id.trim().is_empty() {
            return Err(Error::InvalidEvidence);
        }
        let mut store = self.lock()?;
        let mut next = store.state.clone();
        let id = next.next_turn;
        next.next_turn = id.checked_add(1).ok_or(Error::InvalidEvidence)?;
        next.turns.insert(
            id,
            RecordedTurn {
                id,
                seat_id: seat_id.into(),
                episode_id: episode_id.into(),
                usage: None,
                verification: vec![],
                status: RecordedTurnStatus::Open,
            },
        );
        store.save(next)?;
        Ok(id)
    }
    /// Persist measured usage; an identical callback retry returns false.
    /// # Errors
    /// Rejects unknown/closed turns, conflicting usage, or storage failure.
    pub fn record_usage(&self, id: u64, input_tokens: u64, output_tokens: u64) -> Result<bool> {
        let mut store = self.lock()?;
        let mut next = store.state.clone();
        let turn = next.turns.get_mut(&id).ok_or(Error::InvalidEvidence)?;
        if let Some(old) = &turn.usage {
            return if (old.input_tokens, old.output_tokens) == (input_tokens, output_tokens) {
                Ok(false)
            } else {
                Err(Error::InvalidEvidence)
            };
        }
        if turn.status != RecordedTurnStatus::Open {
            return Err(Error::InvalidEvidence);
        }
        turn.usage = Some(TurnUsage {
            seat_id: turn.seat_id.clone(),
            input_tokens,
            output_tokens,
            wasted: turn.verification.iter().any(|v| !v.passed),
        });
        store.save(next)?;
        Ok(true)
    }
    /// Persist a host checker result before submitting completion.
    /// # Errors
    /// Rejects malformed/mismatched/conflicting evidence or persistence failure.
    pub fn record_verification(&self, id: u64, evidence: VerificationEvidence) -> Result<bool> {
        let mut store = self.lock()?;
        let mut next = store.state.clone();
        let turn = next.turns.get_mut(&id).ok_or(Error::InvalidEvidence)?;
        validate_checker(turn, &evidence)?;
        if let Some(old) = turn
            .verification
            .iter()
            .find(|v| v.binding.node_id == evidence.binding.node_id)
        {
            return if *old == evidence {
                Ok(false)
            } else {
                Err(Error::InvalidEvidence)
            };
        }
        if turn.status != RecordedTurnStatus::Open {
            return Err(Error::InvalidEvidence);
        }
        if !evidence.passed
            && let Some(usage) = &mut turn.usage
        {
            usage.wasted = true;
        }
        turn.verification.push(evidence);
        store.save(next)?;
        Ok(true)
    }
    /// Mark a returned invocation finished; repeated calls are idempotent.
    /// # Errors
    /// Rejects unknown/interrupted IDs or persistence failure.
    pub fn finish_turn(&self, id: u64) -> Result<()> {
        let mut store = self.lock()?;
        let mut next = store.state.clone();
        let turn = next.turns.get_mut(&id).ok_or(Error::InvalidEvidence)?;
        match turn.status {
            RecordedTurnStatus::Interrupted => return Err(Error::InvalidEvidence),
            RecordedTurnStatus::Finished => return Ok(()),
            RecordedTurnStatus::Open => turn.status = RecordedTurnStatus::Finished,
        }
        store.save(next)
    }
    /// Retain unfinished intents as interrupted after stopping their runners.
    ///
    /// Missing usage remains unknown; this operation never retries model calls.
    /// # Errors
    /// Returns a persistence error without silently discarding measured evidence.
    pub fn interrupt_open_turns(&self) -> Result<usize> {
        let mut store = self.lock()?;
        let mut next = store.state.clone();
        let mut count = 0;
        for turn in next
            .turns
            .values_mut()
            .filter(|t| t.status == RecordedTurnStatus::Open)
        {
            turn.status = RecordedTurnStatus::Interrupted;
            if let Some(usage) = &mut turn.usage {
                usage.wasted = true;
            }
            count += 1;
        }
        if count > 0 {
            store.save(next)?;
        }
        Ok(count)
    }
    /// Atomically consume callbacks, redacted observation and dossier for one epoch.
    ///
    /// Repeated epochs return the saved receipt without consuming new evidence.
    /// Stop admission and join runners first: coordinator reads are not atomic.
    /// # Errors
    /// Rejects unfinished turns, skipped epochs, changed replay settings or I/O.
    pub fn checkpoint(
        &self,
        epoch: u64,
        coordinator: &Coordinator,
        memory_health: MemoryHealth,
    ) -> Result<EvidenceEpoch> {
        let mut store = self.lock()?;
        if let Some(saved) = store.state.epochs.get(&epoch) {
            return if saved.dossier.memory_health == memory_health {
                Ok(saved.clone())
            } else {
                Err(Error::InvalidEvidence)
            };
        }
        let expected = store
            .state
            .epochs
            .last_key_value()
            .map_or(Some(1), |(last, _)| last.checked_add(1));
        if Some(epoch) != expected {
            return Err(Error::InvalidEvidence);
        }
        if store
            .state
            .turns
            .values()
            .any(|t| t.status == RecordedTurnStatus::Open)
        {
            return Err(Error::EvidencePending);
        }
        let mut next = store.state.clone();
        let mut observer = Observer::new();
        let mut observation = observer.snapshot(coordinator)?;
        observation
            .transcript
            .retain(|r| next.cursor.is_none_or(|cursor| r.sequence > cursor));
        observation
            .episodes
            .retain(|e| !next.settled.contains(&e.episode_id));
        next.cursor = observer.cursor().or(next.cursor);
        for e in &observation.episodes {
            if matches!(
                e.phase,
                ObservedEpisodePhase::Settled | ObservedEpisodePhase::Failed
            ) {
                next.settled.insert(e.episode_id.clone());
            }
        }
        let ids = next
            .turns
            .keys()
            .filter(|id| !next.consumed.contains(id))
            .copied()
            .collect::<Vec<_>>();
        let receipt = build_epoch(epoch, observation, &ids, &next.turns, memory_health)?;
        next.consumed.extend(ids);
        next.epochs.insert(epoch, receipt.clone());
        store.save(next)?;
        Ok(receipt)
    }
    /// Retained invocation records in allocation order.
    /// # Errors
    /// Returns host failure if a collector lock is poisoned.
    pub fn turns(&self) -> Result<Vec<RecordedTurn>> {
        Ok(self.lock()?.state.turns.values().cloned().collect())
    }
    /// Retained atomic epoch receipts in epoch order.
    /// # Errors
    /// Returns host failure if a collector lock is poisoned.
    pub fn epochs(&self) -> Result<Vec<EvidenceEpoch>> {
        Ok(self.lock()?.state.epochs.values().cloned().collect())
    }
}
/// Derives the dossier from measured callbacks and lists unknown costs separately.
fn build_epoch(
    epoch: u64,
    observation: crate::Observation,
    ids: &[u64],
    turns: &std::collections::BTreeMap<u64, RecordedTurn>,
    health: MemoryHealth,
) -> Result<EvidenceEpoch> {
    let turns = ids
        .iter()
        .map(|id| turns.get(id).ok_or(Error::InvalidEvidence))
        .collect::<Result<Vec<_>>>()?;
    let usage = turns
        .iter()
        .filter_map(|t| t.usage.clone())
        .collect::<Vec<_>>();
    let outcomes = turns
        .iter()
        .flat_map(|t| t.verification.iter())
        .map(|v| VerifierOutcome {
            node_id: v.binding.node_id.clone(),
            status: if v.passed {
                NodeStatus::Verified
            } else {
                NodeStatus::Failed
            },
        })
        .collect::<Vec<_>>();
    Ok(EvidenceEpoch {
        epoch,
        turn_ids: ids.to_vec(),
        interrupted_turns: turns
            .iter()
            .filter(|t| t.status == RecordedTurnStatus::Interrupted)
            .map(|t| t.id)
            .collect(),
        unmetered_turns: turns
            .iter()
            .filter(|t| t.usage.is_none())
            .map(|t| t.id)
            .collect(),
        dossier: Dossier::build(epoch, &observation, &usage, &outcomes, health),
        observation,
    })
}
#[cfg(test)]
mod test;
