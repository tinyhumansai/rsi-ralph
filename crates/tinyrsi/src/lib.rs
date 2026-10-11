//! Host-driven improvement loops: hive proposals and plain scripts.
//!
//! [`run_proposal`] guards a typed hive patch, shadow-evaluates paired private
//! tasks, confirms on fresh draws and asks a host-owned human approval gate.
//! [`Archive`] retains exact language records and packages. [`Observer`] and
//! [`Dossier`] provide read-only evidence; [`Blueprint`] tracks host-verified
//! dependency completion. The host supplies execution, private judges, memory
//! forks and live activation; this library never opens a model connection on
//! its own.
//!
//! [`run_script_loop`] runs the same observe, ideate, execute, score and
//! improve cycle over a plain script with a measurable output, and writes a
//! live [`LoopEvent`] trace.
//!
//! [`Lineage`] is a small in-memory ledger of graded candidates
//! ([`GradeEntry`]) that answers which one is the current incumbent.
//!
//! # Layout
//!
//! - `src/error/` holds the crate-wide [`Error`] enum and the [`Result`] alias
//!   returned by every fallible public function.
//! - Each feature area lives in its own module directory with a `mod.rs`
//!   module root, an optional `types.rs`, and a `test.rs` holding its unit
//!   tests.
//! - Every public item is re-exported from here, so downstream users have a
//!   single predictable surface.
//!
//! # Example
//!
//! ```
//! use tinyrsi::{Error, GradeEntry, Lineage, Verdict};
//!
//! let mut lineage = Lineage::new();
//! let entry = GradeEntry::new(0, "sha256:abc", 0.703, Verdict::Accepted, "baseline");
//! assert!(lineage.append(entry.clone())?);
//! assert_eq!(lineage.append(entry).unwrap_err(), Error::StepOutOfOrder);
//! assert_eq!(lineage.head().map(|e| e.step), Some(0));
//! # Ok::<(), tinyrsi::Error>(())
//! ```

mod archive;
mod blueprint;
#[cfg(feature = "host")]
mod epoch_driver;
mod error;
mod evaluation;
#[cfg(feature = "host")]
mod evidence_journal;
mod experiment;
#[cfg(feature = "host")]
mod host;
mod lineage;
mod memory;
mod observation;
mod orchestrator;
mod process;
mod retreat;
mod retreat_memory;
mod scoring;
mod script_loop;
#[cfg(feature = "host")]
mod suites;
mod trace;
mod verification;

pub use error::{Error, Result};
#[cfg(feature = "host")]
pub use evidence_journal::{EvidenceEpoch, EvidenceJournal, RecordedTurn, RecordedTurnStatus};
pub use lineage::{GradeEntry, Lineage, Verdict};

pub use archive::{
    Archive, DuplicateDisposition, EvaluationEvent, ProposalDeclaration, ProposalDispositionReport,
    ProposalDispositionRow,
};
pub use blueprint::*;
pub use evaluation::{
    Budget, Comparison, EvalDecision, EvalPolicy, EvalRun, JudgeCalibration, QualityRubric,
    VerifierTier, compare, ensure_independent,
};
pub use observation::*;
pub use orchestrator::*;
pub use tinyhivemind_lang;
// Shared proposal data and the language's own definitions, re-exported so
// hosts name one crate.
pub use tinyrsi_types::{
    CandidateState, EvalStage, Operation, Package, Patch, PatchClass, Prediction, Proposal,
    ProposalEvent, ReasonCode, Record, VersionMetadata,
};

pub use retreat::{
    RetreatOutput, RetreatTransport, build_retreat_request, decode_retreat_response,
    generate_retreat,
};

pub use scoring::{
    AnchorLedger, AnchorSignal, FamilyAnchor, ScoreComparison, ScoreEstimate, ScoreHealth,
    ScorePenalties, ScoringPolicy, compare_h, geometric_family_mean, score,
};

pub use retreat::{generate_retreat_with_engine, typed_retreat_request};

pub use archive::NormalizedEvaluationEvent;
#[cfg(feature = "host")]
pub use host::*;
pub use retreat_memory::*;
pub use scoring::ScoringPolicySpec;
pub use verification::*;

#[cfg(feature = "host")]
pub use epoch_driver::*;
pub use experiment::*;
pub use memory::SnapshotMemoryStore;
#[cfg(feature = "host")]
pub use suites::*;
pub use tinyrsi_model::{
    BudgetedModel, Error as ModelError, GenerationMeter, ModelConversant, ModelFuture,
    ModelIdentity, ModelPort, ModelReply, ModelRequest, OpenRouterModel, block_on,
};

pub use archive::NormalizedApprovalEvent;

pub use archive::NormalizedDeclarationEvent;

// The plain-script improvement loop and the append-only trace it emits.
pub use script_loop::{
    CandidateDecision, IncumbentEntry, LedgerCandidate, LoopPolicy, LoopReport, MetricSpec,
    ProposedFile, RunLocation, ScriptCommand, ScriptLedger, ScriptProposal, ScriptSubject, SeedRun,
    Supervisor, SupervisorLimits, parse_metric, run_script_loop,
};
pub use trace::{
    Clock, JsonlTrace, LoopEvent, ManualClock, MetricDirection, MonotonicClock, NullTrace, RunGoal,
    ScoreDecision, StopReason, TraceRecord, TraceSink, VecTrace,
};
