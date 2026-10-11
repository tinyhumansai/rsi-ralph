//! Host-owned checkers: did a candidate's artifact actually pass?
//!
//! A checker is evidence a model cannot talk its way past. [`VerifierRegistry`]
//! freezes the host's [`HostVerifier`] declarations: an executed command, a
//! golden output, a JSON schema, a reference answer, or a Lean theorem built
//! by a pinned Lake toolchain. It checks each [`ArtifactSubmission`] against
//! the [`VerificationBinding`] it was issued for. [`VerifiedRunner`] completes a
//! coordinator episode only after every node of a [`Blueprint`] has passed its
//! checker. [`process`] is the bounded child-process runner both checkers and
//! the script loop use.
//!
//! [`VerifierScorer`] puts a checker behind the `tinyrsi_score::Scorer` port,
//! so a pass/fail check can be combined with metrics and judges.
//!
//! # Example
//!
//! ```
//! use tinyrsi_verify::{
//!     ArtifactSubmission, HostVerifier, VerificationBinding, VerifierDeclaration,
//!     VerifierRegistry,
//! };
//!
//! let registry = VerifierRegistry::new(vec![VerifierDeclaration {
//!     node_id: "answer".into(),
//!     owner_seat: "solver".into(),
//!     verifier: HostVerifier::Golden { expected: b"42".to_vec() },
//! }])?;
//! let binding = VerificationBinding {
//!     episode_id: "ep-1".into(),
//!     seat_id: "solver".into(),
//!     node_id: "answer".into(),
//!     artifact_id: "a-1".into(),
//!     transcript_refs: vec![1],
//! };
//! let submission = ArtifactSubmission { binding: binding.clone(), content: b"42".to_vec() };
//! assert!(registry.verify(&binding, &submission)?.passed);
//! # Ok::<(), tinyrsi_verify::Error>(())
//! ```
//!
//! # What this crate does not hold
//!
//! No scoring decisions and no loop. A checker reports pass or fail and its
//! evidence; deciding what that means for a candidate is `tinyrsi-score`'s job.

pub mod process;

mod blueprint;
mod error;
mod scorer;
mod verification;

pub use blueprint::*;
pub use error::{Error, Result};
pub use scorer::VerifierScorer;
pub use verification::*;
