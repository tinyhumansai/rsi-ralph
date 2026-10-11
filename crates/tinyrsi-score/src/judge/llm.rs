//! A judge over a bounded model port.
use super::{gate, prompt};
use crate::{Assessment, Error, JudgeCalibration, Result, Scorer};
use serde_json::{Value, json};
use std::sync::Arc;
use tinyrsi_model::{ModelPort, ModelRequest, block_on};
use tinyrsi_types::Goal;

/// Name of the forced grading tool.
const TOOL: &str = "grade";

const SYSTEM: &str = "You are an impartial evaluator in an automated improvement loop. \
Read the goal, its criteria and the subject, then call the grade tool exactly once. \
score is how well the subject achieves the goal, from 0 (not at all) to 10 (fully, \
every criterion met). Judge only what is in the subject; do not reward length, \
confidence or claims that are not shown.";

/// Per-call limits for judge requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgeLimits {
    /// Total tokens reserved per call, prompt included.
    pub call_tokens: u64,
    /// Output tokens per call.
    pub output_tokens: u64,
    /// Per-call timeout in milliseconds, at most 300 000.
    pub timeout_ms: u64,
}

impl Default for JudgeLimits {
    fn default() -> Self {
        Self {
            call_tokens: 16_384,
            output_tokens: 512,
            timeout_ms: 120_000,
        }
    }
}

/// Grades a subject rendered as text, with independent raters per seed.
#[derive(Clone)]
pub struct LlmJudge {
    port: Arc<dyn ModelPort>,
    raters: u32,
    limits: JudgeLimits,
    calibration: Option<JudgeCalibration>,
}

impl LlmJudge {
    /// A judge asking `raters` (1 to 255) independent grades per seed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidEvaluation`] for a rater count outside 1 to
    /// 255.
    pub fn new(port: Arc<dyn ModelPort>, raters: u32, limits: JudgeLimits) -> Result<Self> {
        if !(1..=255).contains(&raters) {
            return Err(Error::InvalidEvaluation);
        }
        Ok(Self {
            port,
            raters,
            limits,
            calibration: None,
        })
    }

    /// Attaches the host's calibration of this judge against human labels.
    /// Its assessments are accept-eligible only when the calibration passes.
    #[must_use]
    pub fn calibrated(mut self, calibration: JudgeCalibration) -> Self {
        self.calibration = Some(calibration);
        self
    }

    fn schema() -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["score", "rationale"],
            "properties": {
                "score": {"type": "number", "minimum": 0, "maximum": 10},
                "rationale": {"type": "string", "description": "Which criteria are met or missed, briefly."}
            }
        })
    }

    /// One rater's grade in `[0, 1]`, its rationale and its measured tokens.
    fn rate(&self, text: &str, seed: u64) -> Result<(f64, String, u64)> {
        let request = ModelRequest::new(
            SYSTEM.into(),
            text.into(),
            seed,
            self.limits.call_tokens,
            self.limits.output_tokens,
            self.limits.timeout_ms,
        )?
        .with_tool(TOOL.into(), Self::schema())?;
        let reply = block_on(self.port.generate(request.clone()))?;
        request.check(&reply)?;
        let tokens = reply.input_tokens.saturating_add(reply.output_tokens);
        let value: Value =
            serde_json::from_slice(&reply.content).map_err(|_| Error::JudgeMalformed)?;
        let score = value
            .get("score")
            .and_then(Value::as_f64)
            .filter(|s| (0.0..=10.0).contains(s))
            .ok_or(Error::JudgeMalformed)?;
        let rationale = value
            .get("rationale")
            .and_then(Value::as_str)
            .ok_or(Error::JudgeMalformed)?;
        Ok((score / 10.0, rationale.to_owned(), tokens))
    }
}

/// The median of a non-empty slice.
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        f64::midpoint(values[mid - 1], values[mid])
    } else {
        values[mid]
    }
}

impl Scorer<str> for LlmJudge {
    /// # Errors
    ///
    /// Returns [`Error::InvalidEvaluation`] for a malformed goal or no seeds,
    /// [`Error::JudgeMalformed`] for a reply that is not one grade, and
    /// [`Error::Model`] when a call fails or overspends.
    fn assess(&self, subject: &str, goal: &Goal, seeds: &[u64]) -> Result<Assessment> {
        if !goal.is_well_formed() || seeds.is_empty() {
            return Err(Error::InvalidEvaluation);
        }
        let text = prompt(goal, subject);
        let mut qualities = Vec::with_capacity(seeds.len());
        let mut rationale = None;
        let mut tokens = 0_u64;
        for seed in seeds {
            let mut grades = Vec::with_capacity(self.raters as usize);
            for rater in 0..self.raters {
                let (grade, why, spent) =
                    self.rate(&text, seed.wrapping_mul(256).wrapping_add(u64::from(rater)))?;
                tokens = tokens.saturating_add(spent);
                rationale.get_or_insert(why);
                grades.push(grade);
            }
            qualities.push(median(&mut grades));
        }
        Ok(Assessment {
            eligible: gate(self.calibration.as_ref(), &qualities),
            qualities,
            metric: None,
            rationale,
            tokens,
        })
    }
}

impl std::fmt::Debug for LlmJudge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmJudge")
            .field("raters", &self.raters)
            .field("limits", &self.limits)
            .field("calibrated", &self.calibration.is_some())
            .finish_non_exhaustive()
    }
}
