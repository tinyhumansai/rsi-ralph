//! The strike ledger: a pure projection of archive events onto patch shapes.
//!
//! `Archive::penalties` reads `HARMFUL` stops, confident normalized
//! regressions and host revocations into a `PenaltyReport`. Host failures,
//! inconclusive evidence, approval denials and prefilter stops are not
//! strikes. The orchestrator's prefilter asks `PenaltyReport::cooled` before
//! spending any evaluation budget; see `README.md`.
mod types;
use super::Event;
use super::dispositions::reason;
use crate::ReasonCode;
use std::fmt;
use tinyhivemind_lang::lineage::Record;
use tinyhivemind_lang::patch::{Operation, Patch};
pub use types::*;

impl PatchShape {
    /// The shape of `patch`: its class and sorted `(tag, target)` pairs.
    #[must_use]
    pub fn of(patch: &Patch) -> Self {
        let mut targets: Vec<(String, String)> = patch
            .operations
            .iter()
            .map(|operation| match operation {
                Operation::Json { pointer, .. } => ("json".into(), pointer.clone()),
                Operation::Document { path, .. } => ("document".into(), path.clone()),
                Operation::Prompt { path, .. } => ("prompt".into(), path.clone()),
                Operation::Context { path, item, .. } => {
                    ("context".into(), format!("{path}#{item}"))
                }
                Operation::Learning {
                    identity, entry, ..
                } => ("learning".into(), format!("{identity}#{entry}")),
            })
            .collect();
        targets.sort();
        targets.dedup();
        Self {
            class: patch.class,
            targets,
        }
    }
}

/// `class: tag target, tag target`, for example
/// `context: context context/main.md#method`.
impl fmt::Display for PatchShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let class = match self.class {
            tinyhivemind_lang::patch::PatchClass::Prompt => "prompt",
            tinyhivemind_lang::patch::PatchClass::Context => "context",
            tinyhivemind_lang::patch::PatchClass::Memory => "memory",
            tinyhivemind_lang::patch::PatchClass::Roster => "roster",
            tinyhivemind_lang::patch::PatchClass::Topology => "topology",
            tinyhivemind_lang::patch::PatchClass::Policy => "policy",
            tinyhivemind_lang::patch::PatchClass::Goal => "goal",
        };
        write!(f, "{class}:")?;
        for (index, (tag, target)) in self.targets.iter().enumerate() {
            let separator = if index == 0 { " " } else { ", " };
            write!(f, "{separator}{tag} {target}")?;
        }
        Ok(())
    }
}

impl PenaltyReport {
    /// Whether `shape` is cooled at `epoch` under `policy`.
    ///
    /// Only strikes from epochs `e <= epoch` with `epoch - e < window_epochs`
    /// count. A `Harmful` or `Revoked` strike among them cools the shape;
    /// otherwise at least `policy.strikes` of them must.
    #[must_use]
    pub fn cooled(&self, shape: &PatchShape, epoch: u64, policy: &PenaltyPolicy) -> bool {
        if policy.strikes == 0 || policy.window_epochs == 0 {
            return false;
        }
        let recent: Vec<&Strike> = self
            .strikes
            .iter()
            .filter(|s| {
                s.shape == *shape && s.epoch <= epoch && epoch - s.epoch < policy.window_epochs
            })
            .collect();
        recent
            .iter()
            .any(|s| matches!(s.kind, StrikeKind::Harmful | StrikeKind::Revoked))
            || u32::try_from(recent.len()).unwrap_or(u32::MAX) >= policy.strikes
    }
}

pub(super) fn project(events: &[Event]) -> PenaltyReport {
    let mut records: Vec<&Record> = Vec::new();
    // Normalized evidence by (epoch, patch): its upper delta bound.
    let mut uppers: Vec<(u64, &Patch, f64)> = Vec::new();
    let mut strikes = Vec::new();
    let mut proposals = 0;
    for event in events {
        match event {
            Event::Candidate { record, .. } => records.push(record),
            Event::NormalizedEvaluation { evaluation } => uppers.push((
                evaluation.epoch,
                &evaluation.proposal.patch,
                evaluation.comparison.upper,
            )),
            Event::Proposal { proposal } => {
                let regressed = || {
                    uppers
                        .iter()
                        .rev()
                        .find(|(epoch, patch, _)| {
                            *epoch == proposal.epoch && **patch == proposal.patch
                        })
                        .is_some_and(|(_, _, upper)| *upper < 0.0)
                };
                let kind = match reason(&proposal.reason) {
                    Some(ReasonCode::Harmful) => Some(StrikeKind::Harmful),
                    Some(ReasonCode::NoGain | ReasonCode::NotReplicated) if regressed() => {
                        Some(StrikeKind::Regressed)
                    }
                    _ => None,
                };
                if let Some(kind) = kind {
                    strikes.push(Strike {
                        shape: PatchShape::of(&proposal.patch),
                        epoch: proposal.epoch,
                        kind,
                        proposal: Some(proposals),
                    });
                }
                proposals += 1;
            }
            Event::Revocation {
                candidate_id,
                epoch,
            } => {
                if let Some(record) = records.iter().find(|r| r.id == *candidate_id) {
                    strikes.push(Strike {
                        shape: PatchShape::of(&record.patch),
                        epoch: *epoch,
                        kind: StrikeKind::Revoked,
                        proposal: None,
                    });
                }
            }
            _ => {}
        }
    }
    PenaltyReport { strikes }
}
