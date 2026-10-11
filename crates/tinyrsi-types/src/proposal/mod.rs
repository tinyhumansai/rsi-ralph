//! What a proposer submits and why the host stopped or selected it.
//!
//! [`Proposal`] is strict: unknown fields are rejected, so a proposer cannot
//! smuggle extra claims past the host. [`ProposalEvent`] is the audit row for an
//! attempt that never produced an evaluated record.
mod types;

pub use types::{Prediction, Proposal, ProposalEvent, ReasonCode};

#[cfg(test)]
mod test;
