//! How a candidate failed runs its incumbent completed.
//!
//! [`CandidateFailure`] is what a host evaluator reports when the incumbent
//! arm of a paired draw ran to completion but the candidate arm did not. It
//! carries only a coarse [`CandidateFailureKind`] and counts: no stderr, model
//! reply or private task text, so it is safe to retain as archive evidence.
mod types;

pub use types::{CandidateFailure, CandidateFailureKind};

#[cfg(test)]
mod test;
