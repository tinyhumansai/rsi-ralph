//! Scoring: is a candidate closer to the goal than the incumbent?
//!
//! [`compare`] decides a paired incumbent-versus-candidate comparison of
//! qualities in `[0, 1]`, and [`compare_h`] does the same for the normalized H
//! score with health penalties and family anchors. [`run_experiment`] runs a
//! preregistered original-versus-evolved protocol on top of them.

mod error;
mod evaluation;
mod experiment;
mod scoring;

pub use error::{Error, Result};
pub use evaluation::{
    Budget, Comparison, EvalDecision, EvalPolicy, EvalRun, JudgeCalibration, QualityRubric,
    VerifierTier, compare, ensure_independent,
};
pub use experiment::*;
pub use scoring::{
    AnchorLedger, AnchorSignal, FamilyAnchor, ScoreComparison, ScoreEstimate, ScoreHealth,
    ScorePenalties, ScoringPolicy, ScoringPolicySpec, compare_h, geometric_family_mean, score,
};
