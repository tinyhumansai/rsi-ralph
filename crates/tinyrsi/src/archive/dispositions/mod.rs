//! Safe duplicate subtypes derived only from the archive prefix at each stop.
mod types;
use super::{Event, ProposalEvent};
use crate::ReasonCode;
use tinyhivemind_lang::lineage::{Record, Verdict};
pub use types::*;

/// Recognize both existing Rust archive spellings and the public serde wire form.
fn reason(text: &str) -> Option<ReasonCode> {
    serde_json::from_value(serde_json::Value::String(text.into()))
        .ok()
        .or_else(|| {
            [
                ReasonCode::Untestable,
                ReasonCode::Ungrounded,
                ReasonCode::WrongClass,
                ReasonCode::Guard,
                ReasonCode::Unformalizable,
                ReasonCode::Duplicate,
                ReasonCode::NoGain,
                ReasonCode::Inconclusive,
                ReasonCode::NotReplicated,
                ReasonCode::ApprovalDenied,
                ReasonCode::HostFailure,
                ReasonCode::Accepted,
            ]
            .into_iter()
            .find(|r| format!("{r:?}") == text)
        })
}

/// Failed infrastructure, duplicates and unvalidated accepted claims do not prove rejection.
fn terminal_rejection(text: &str) -> bool {
    reason(text).is_some_and(|r| {
        !matches!(
            r,
            ReasonCode::HostFailure | ReasonCode::Duplicate | ReasonCode::Accepted
        )
    })
}

pub(super) fn project(events: &[Event]) -> ProposalDispositionReport {
    let mut records: Vec<&Record> = Vec::new();
    let mut proposals: Vec<&ProposalEvent> = Vec::new();
    let mut rows = Vec::new();
    for (event_index, event) in events.iter().enumerate() {
        match event {
            Event::Candidate { record, .. } => records.push(record),
            Event::Proposal { proposal } => {
                let coarse = reason(&proposal.reason);
                let duplicate = (coarse == Some(ReasonCode::Duplicate)).then(|| {
                    if records
                        .iter()
                        .any(|r| r.patch == proposal.patch && r.verdict == Verdict::Accepted)
                    {
                        DuplicateDisposition::Accepted
                    } else if records
                        .iter()
                        .any(|r| r.patch == proposal.patch && r.verdict == Verdict::Rejected)
                        || proposals
                            .iter()
                            .any(|p| p.patch == proposal.patch && terminal_rejection(&p.reason))
                    {
                        DuplicateDisposition::Rejected
                    } else {
                        DuplicateDisposition::Unresolved
                    }
                });
                rows.push(ProposalDispositionRow {
                    event_index,
                    epoch: proposal.epoch,
                    reason: coarse,
                    duplicate,
                });
                proposals.push(proposal);
            }
            _ => {}
        }
    }
    ProposalDispositionReport { version: 1, rows }
}
