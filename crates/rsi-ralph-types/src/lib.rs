//! Proposal and candidate data shared across the `rsi-ralph` crates.
//!
//! A *proposal* is what a proposer submits: a typed hive [`Patch`], the
//! mechanism it claims, and a [`Prediction`] it commits to before anything is
//! run. A *candidate* is the package and pinned memory that proposal produces
//! ([`CandidateState`]), evaluated in [`EvalStage`]s. [`ReasonCode`] records
//! why a candidate stopped or was selected, and [`ProposalEvent`] keeps an
//! attempt that never became an evaluated record. A [`Goal`] states in words
//! what "better" means, for the judges that score against it.
//!
//! These types sit below every other `rsi-ralph` crate. Scoring, evolution, the
//! loops and the host runtime all name them, so they live here rather than in
//! any one of those crates.
//!
//! # Example
//!
//! ```
//! use rsi_ralph_types::{Prediction, ReasonCode};
//!
//! let prediction = Prediction {
//!     metric: "normalized_h".into(),
//!     minimum_gain: 0.02,
//!     nodes: vec!["parse".into()],
//! };
//! assert!(prediction.minimum_gain > 0.0);
//! assert_eq!(serde_json::to_value(ReasonCode::NoGain)?, "NO_GAIN");
//! # Ok::<(), serde_json::Error>(())
//! ```
//!
//! # What this crate does not hold
//!
//! No behavior and no error type: nothing here can fail. Deciding whether a
//! proposal is grounded, testable or better than the incumbent belongs to the
//! crates that do that work.

mod candidate;
mod failure;
mod goal;
mod proposal;

pub use candidate::{CandidateState, EvalStage};
pub use failure::{CandidateFailure, CandidateFailureKind};
pub use goal::Goal;
pub use proposal::{Prediction, Proposal, ProposalEvent, ReasonCode};

// The language's own definitions, re-exported so every crate names
// the same types.
pub use tinyhivemind_lang::Package;
pub use tinyhivemind_lang::lineage::{Record, VersionMetadata};
pub use tinyhivemind_lang::patch::{Operation, Patch, PatchClass};
