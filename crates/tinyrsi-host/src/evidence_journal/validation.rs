//! Replay validation of invocation identities and derived epoch dossiers.
use super::{RecordedTurn, RecordedTurnStatus, State, build_epoch};
use crate::{Error, Result, VerificationEvidence};
use std::collections::BTreeSet;
/// Checks invocation identity, ordered references and binary deterministic results.
pub(super) fn validate_checker(turn: &RecordedTurn, evidence: &VerificationEvidence) -> Result<()> {
    let binding = &evidence.binding;
    if binding.seat_id != turn.seat_id
        || binding.episode_id != turn.episode_id
        || binding.node_id.trim().is_empty()
        || binding.artifact_id.trim().is_empty()
        || binding
            .transcript_refs
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || evidence.quality.to_bits() != f64::from(u8::from(evidence.passed)).to_bits()
        || evidence.artifact_digest.len() != 64
        || !evidence
            .artifact_digest
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || evidence.checker_identity.len() != 71
        || !evidence.checker_identity.starts_with("sha256:")
        || !evidence.checker_identity[7..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(Error::InvalidEvidence);
    }
    Ok(())
}
/// Recomputes receipts and verifies contiguous IDs and exactly-once consumption.
pub(super) fn validate(state: &State) -> Result<()> {
    let check = || -> Result<()> {
        if state.version != 1
            || state.next_turn
                != u64::try_from(state.turns.len())
                    .ok()
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Error::InvalidEvidence)?
        {
            return Err(Error::InvalidEvidence);
        }
        for (expected_turn, (id, turn)) in (1..).zip(&state.turns) {
            if *id != expected_turn
                || turn.id != *id
                || turn.seat_id.trim().is_empty()
                || turn.episode_id.trim().is_empty()
            {
                return Err(Error::InvalidEvidence);
            }
            let mut nodes = BTreeSet::new();
            for evidence in &turn.verification {
                validate_checker(turn, evidence)?;
                if !nodes.insert(&evidence.binding.node_id) {
                    return Err(Error::InvalidEvidence);
                }
            }
            if let Some(usage) = &turn.usage
                && (usage.seat_id != turn.seat_id
                    || usage.wasted
                        != (turn.status == RecordedTurnStatus::Interrupted
                            || turn.verification.iter().any(|v| !v.passed)))
            {
                return Err(Error::InvalidEvidence);
            }
        }
        let mut consumed = BTreeSet::new();
        let mut expected_epoch = 1;
        let mut cursor = None;
        let mut settled = BTreeSet::new();
        for (epoch, receipt) in &state.epochs {
            if *epoch != expected_epoch
                || receipt.epoch != *epoch
                || receipt.turn_ids.windows(2).any(|p| p[0] >= p[1])
            {
                return Err(Error::InvalidEvidence);
            }
            expected_epoch = expected_epoch
                .checked_add(1)
                .ok_or(Error::InvalidEvidence)?;
            for id in &receipt.turn_ids {
                let expected_id = u64::try_from(consumed.len())
                    .ok()
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Error::InvalidEvidence)?;
                if *id != expected_id
                    || !consumed.insert(*id)
                    || state
                        .turns
                        .get(id)
                        .is_none_or(|t| t.status == RecordedTurnStatus::Open)
                {
                    return Err(Error::InvalidEvidence);
                }
            }
            for row in &receipt.observation.transcript {
                if cursor.is_some_and(|c| row.sequence <= c) {
                    return Err(Error::InvalidEvidence);
                }
                cursor = Some(row.sequence);
            }
            for e in &receipt.observation.episodes {
                if settled.contains(&e.episode_id) {
                    return Err(Error::InvalidEvidence);
                }
                if matches!(
                    e.phase,
                    crate::ObservedEpisodePhase::Settled | crate::ObservedEpisodePhase::Failed
                ) {
                    settled.insert(e.episode_id.clone());
                }
            }
            if *receipt
                != build_epoch(
                    *epoch,
                    receipt.observation.clone(),
                    &receipt.turn_ids,
                    &state.turns,
                    receipt.dossier.memory_health,
                )?
            {
                return Err(Error::InvalidEvidence);
            }
        }
        if consumed != state.consumed || cursor != state.cursor || settled != state.settled {
            return Err(Error::InvalidEvidence);
        }
        Ok(())
    };
    check().map_err(|_| Error::ArchiveCorrupt)
}
