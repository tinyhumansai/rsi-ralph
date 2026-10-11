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
//!   returned by every fallible function defined in this crate. Items
//!   re-exported from lower crates keep their own error type:
//!   [`ModelRequest::new`] and the other model APIs return [`ModelError`].
//!   Every such error converts into [`Error`] with `?`, landing on the variant
//!   this crate always reported.
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
#[cfg(feature = "host")]
mod epoch_driver;
mod error;
#[cfg(feature = "host")]
mod evidence_journal;
#[cfg(feature = "host")]
mod host;
mod lineage;
mod memory;
mod observation;
mod orchestrator;
mod script_loop;
#[cfg(feature = "host")]
mod suites;

pub use error::{Error, Result};
#[cfg(feature = "host")]
pub use evidence_journal::{EvidenceEpoch, EvidenceJournal, RecordedTurn, RecordedTurnStatus};
pub use lineage::{GradeEntry, Lineage, Verdict};

pub use archive::{
    Archive, DuplicateDisposition, EvaluationEvent, ProposalDeclaration, ProposalDispositionReport,
    ProposalDispositionRow,
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

pub use archive::NormalizedEvaluationEvent;
#[cfg(feature = "host")]
pub use host::*;
// Evolution: the Evolver port, the script supervisor, retreat and its memory.
pub use tinyrsi_evolve::{
    DurableRetreatMemory, Error as EvolveError, EvolveContext, Evolver, MemoryLimits, Proposed,
    ProposedFile, RetreatBrief, RetreatDossier, RetreatEvolver, RetreatMemoryOutcome,
    RetreatOutcomeStatus, RetreatOutput, RetreatTransport, ScriptBrief, ScriptProposal,
    ScriptSupervisor, ScriptedEvolver, SupervisorLimits, build_retreat_request,
    decode_retreat_response, generate_retreat, generate_retreat_with_engine, typed_retreat_request,
};
// Checkers: blueprints, verifier registry, verified completion.
pub use tinyrsi_verify::{
    ArtifactFuture, ArtifactProducer, ArtifactSubmission, Blueprint, BlueprintNode, CommandSpec,
    Error as VerifyError, HostVerifier, NodeStatus, VerificationBinding, VerificationEvidence,
    VerificationObserver, VerifiedRunner, Verifier, VerifierDeclaration, VerifierKind,
    VerifierRegistry, VerifierScorer,
};

#[cfg(feature = "host")]
pub use epoch_driver::*;
// Scoring: paired comparison, normalized H, experiments.
pub use memory::SnapshotMemoryStore;
#[cfg(feature = "host")]
pub use suites::*;
pub use tinyrsi_model::{
    BudgetedModel, Error as ModelError, GenerationMeter, ModelConversant, ModelFuture,
    ModelIdentity, ModelPort, ModelReply, ModelRequest, OpenRouterModel, block_on,
};
pub use tinyrsi_score::{
    AblationEstimate, AttemptEvidence, AttemptStatus, EvidenceSource, ExperimentAblation,
    ExperimentArm, ExperimentExecutor, ExperimentProtocol, ExperimentProtocolSpec,
    ExperimentReport, ExperimentUncertainty, ExperimentVerdict, SuiteFingerprints, TrialEvidence,
    TrialFailure, TrialRequest, TrialSeeds, TrialSummary, run_experiment,
};
pub use tinyrsi_score::{
    AnchorLedger, AnchorSignal, Budget, Comparison, Error as ScoreError, EvalDecision, EvalPolicy,
    EvalRun, FamilyAnchor, JudgeCalibration, QualityRubric, ScoreComparison, ScoreEstimate,
    ScoreHealth, ScorePenalties, ScoringPolicy, ScoringPolicySpec, VerifierTier, compare,
    compare_h, ensure_independent, geometric_family_mean, score,
};
pub use tinyrsi_score::{
    Assessment, BackroomsJudge, Composite, Decision, Goal, JudgeLimits, LlmJudge, MetricEvidence,
    MetricScorer, Projected, Scorer, ScriptedScorer, decide,
};

pub use archive::NormalizedApprovalEvent;

pub use archive::NormalizedDeclarationEvent;

// The plain-script improvement loop and the append-only trace it emits.
pub use script_loop::{
    CandidateDecision, IncumbentEntry, IntervalUnits, LedgerCandidate, LoopPolicy, LoopReport,
    MetricSpec, RunLocation, ScriptCandidate, ScriptCommand, ScriptLedger, ScriptSubject, SeedRun,
    parse_metric, run_script_loop, run_script_loop_with,
};
pub use tinyrsi_trace::{
    Clock, Error as TraceError, FanoutTrace, IntervalUnits, JsonlTrace, LoopEvent, ManualClock,
    MetricDirection, MonotonicClock, NullTrace, RunGoal, ScoreDecision, StopReason, TraceRecord,
    TraceSink, TracingTrace, VecTrace,
};
