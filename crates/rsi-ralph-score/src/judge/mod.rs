//! Language models as judges of how close a subject is to a goal.
//!
//! Many goals have no metric: "explain the proof clearly", "bring this file up
//! to the team's standard". A judge reads the [`Goal`] and the subject and
//! grades it. Two judges share the prompt and the calibration gate here:
//!
//! - [`LlmJudge`] asks a [`rsi_ralph_model::ModelPort`] for a forced `grade`
//!   tool call (0 to 10) from several independent raters per seed and keeps
//!   the median.
//! - [`BackroomsJudge`] runs the consensual-assessment judge that tinybackrooms
//!   uses for its own ideas: independent raters on a 1-to-7 scale through any
//!   tinybackrooms [`tinybackrooms::Conversant`], keeping the mean.
//!
//! A judge's assessment is accept-eligible only with a [`JudgeCalibration`]
//! that meets its threshold. Without one it still scores, but
//! [`crate::decide`] never accepts on its evidence alone.
mod backrooms;
mod llm;

use crate::JudgeCalibration;
pub use backrooms::BackroomsJudge;
#[cfg(test)]
use llm::rater_seed;
pub use llm::{JudgeLimits, LlmJudge};
use std::fmt::Write as _;
use rsi_ralph_types::Goal;

/// The user message every judge reads: goal, criteria, then the subject.
fn prompt(goal: &Goal, subject: &str) -> String {
    let mut text = format!("# Goal\n{}\n", goal.statement.trim());
    if !goal.criteria.is_empty() {
        text.push_str("\n# Criteria\n");
        for criterion in &goal.criteria {
            let _ = writeln!(text, "- {}", criterion.trim());
        }
    }
    let _ = write!(text, "\n# Subject\n{}\n", subject.trim_end());
    text
}

/// The eligible qualities: the judge's own, when its calibration passes.
fn gate(calibration: Option<&JudgeCalibration>, qualities: &[f64]) -> Option<Vec<f64>> {
    calibration
        .is_some_and(JudgeCalibration::passes)
        .then(|| qualities.to_vec())
}

#[cfg(test)]
mod test;
