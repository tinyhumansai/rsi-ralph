//! Scoring: is a candidate closer to the goal than the incumbent?
//!
//! Goals are often vague, so there is no loss function. A [`Scorer`] turns a
//! subject into one quality per seed, from whatever evidence fits the goal:
//!
//! - [`MetricScorer`]: a measured metric, such as a solver's error.
//! - [`LlmJudge`]: independent model graders reading the [`Goal`].
//! - [`BackroomsJudge`]: tinybackrooms' consensual-assessment raters.
//! - [`ScriptedScorer`]: any deterministic check written as a closure.
//! - [`Composite`]: a weighted mix, with [`Projected`] to score one view of
//!   a richer subject.
//!
//! [`decide`] is the decision rule for per-seed assessments; [`sign_test`]
//! decides per-item pass/fail benchmarks. It compares the incumbent's and the
//! candidate's assessments with [`compare`]. An `Accept` must also hold on
//! accept-eligible evidence alone, so an uncalibrated judge never decides by
//! itself. [`compare_h`] decides the normalized H score used by the hive
//! funnel, and [`run_experiment`] runs a preregistered original-versus-evolved
//! protocol on top of both.
//!
//! # Example
//!
//! ```
//! use rsi_ralph_score::{EvalPolicy, Goal, MetricDirection, MetricScorer, ScoreDecision, Scorer, decide};
//!
//! let scorer = MetricScorer::new(MetricDirection::Minimize);
//! let goal = Goal::new("fit the curve");
//! let seeds = [1, 2, 3];
//! let incumbent = scorer.assess(&[0.9, 1.0, 1.1], &goal, &seeds)?;
//! let candidate = scorer.assess(&[0.1, 0.2, 0.1], &goal, &seeds)?;
//! let policy = EvalPolicy {
//!     margin: 0.0,
//!     standard_error_multiplier: 2.0,
//!     family_regression_limit: 1.0,
//!     cost_tolerance: 0.0,
//! };
//! let decision = decide(&incumbent, &candidate, &seeds, &policy)?;
//! assert_eq!(decision.decision, ScoreDecision::Accept);
//! assert!((decision.estimate - 0.8667).abs() < 1e-3); // metric units
//! # Ok::<(), rsi_ralph_score::Error>(())
//! ```
//!
//! # What this crate does not hold
//!
//! No process execution and no coordinator: running a candidate to measure it
//! belongs to the loop or to `rsi-ralph-verify`, and this crate only judges
//! evidence it is handed.

mod composite;
mod error;
mod evaluation;
mod experiment;
mod judge;
mod metric;
mod port;
mod scoring;
mod scripted;
mod sign;

pub use composite::{Composite, Projected};
pub use error::{Error, Result};
pub use evaluation::{
    Budget, Comparison, EvalDecision, EvalPolicy, EvalRun, JudgeCalibration, QualityRubric,
    VerifierTier, compare, ensure_independent,
};
pub use experiment::*;
pub use judge::{BackroomsJudge, JudgeLimits, LlmJudge};
pub use metric::MetricScorer;
pub use port::{Assessment, Decision, MetricEvidence, Scorer, decide, dominates};
pub use scoring::{
    AnchorLedger, AnchorSignal, FamilyAnchor, ScoreComparison, ScoreEstimate, ScoreHealth,
    ScorePenalties, ScoringPolicy, ScoringPolicySpec, compare_h, geometric_family_mean, score,
};
pub use scripted::ScriptedScorer;
pub use sign::{SignTest, sign_test, sign_test_p};
pub use rsi_ralph_trace::{MetricDirection, ScoreDecision};
pub use rsi_ralph_types::Goal;
