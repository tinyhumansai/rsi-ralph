//! The tinybackrooms consensual-assessment judge, aimed at a goal.
use super::{gate, prompt};
use crate::{Assessment, Error, JudgeCalibration, Result, Scorer};
use serde_json::{Value, json};
use std::sync::Arc;
use tinybackrooms::{Conversant, Message, Seat, StructuredRequest};
use tinyrsi_model::block_on;
use tinyrsi_types::Goal;

/// Name of the forced rating tool.
const TOOL: &str = "rate_subject";

const SYSTEM: &str = "You are one of several independent raters. Read the goal, its \
criteria and the subject, rate it, and call the rate_subject tool once. goal_fit: 1 means \
the subject does not serve the goal at all, 7 means it fully achieves it with every \
criterion met. novelty, when asked for: 1 means a stock approach anyone would take, 7 \
means nothing like it exists.";

/// Independent raters grading a subject 1 to 7 through a tinybackrooms
/// backend, averaged as in tinybackrooms' own creativity judge.
///
/// With [`BackroomsJudge::with_novelty`], each rating is `goal_fit ×
/// novelty`, the tinybackrooms quality gate that keeps a merely strange or a
/// merely adequate subject from ranking well.
#[derive(Clone)]
pub struct BackroomsJudge {
    backend: Arc<dyn Conversant>,
    raters: u32,
    model: Option<String>,
    novelty: bool,
    calibration: Option<JudgeCalibration>,
}

impl BackroomsJudge {
    /// A judge of `raters` (1 to 255) independent ratings per seed.
    ///
    /// The backend's futures are driven on the calling thread, so it must not
    /// need a specific async runtime.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidEvaluation`] for a rater count outside 1 to
    /// 255.
    pub fn new(backend: Arc<dyn Conversant>, raters: u32) -> Result<Self> {
        if !(1..=255).contains(&raters) {
            return Err(Error::InvalidEvaluation);
        }
        Ok(Self {
            backend,
            raters,
            model: None,
            novelty: false,
            calibration: None,
        })
    }

    /// Asks the backend for `model` instead of its default.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Multiplies goal fit by novelty, for goals that ask for originality.
    #[must_use]
    pub fn with_novelty(mut self) -> Self {
        self.novelty = true;
        self
    }

    /// Attaches the host's calibration of this judge against human labels.
    #[must_use]
    pub fn calibrated(mut self, calibration: JudgeCalibration) -> Self {
        self.calibration = Some(calibration);
        self
    }

    fn schema(&self) -> Value {
        let scale = json!({"type": "integer", "minimum": 1, "maximum": 7});
        let mut required = vec!["goal_fit"];
        let mut properties = json!({"goal_fit": scale});
        if self.novelty {
            required.push("novelty");
            properties["novelty"] = scale;
        }
        json!({"type": "object", "required": required, "properties": properties})
    }

    /// One rating in `[0, 1]`, or `None` when the answer is unusable.
    fn parse(&self, value: &Value) -> Option<f64> {
        let scale = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_f64)
                .filter(|v| (1.0..=7.0).contains(v))
                .map(|v| (v - 1.0) / 6.0)
        };
        let fit = scale("goal_fit")?;
        if self.novelty {
            Some(fit * scale("novelty")?)
        } else {
            Some(fit)
        }
    }
}

impl Scorer<str> for BackroomsJudge {
    /// Raters whose call fails or whose answer is unusable are skipped, as in
    /// tinybackrooms.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidEvaluation`] for a malformed goal or no seeds,
    /// [`Error::HostFailed`] when every rater's call failed for a seed, and
    /// [`Error::JudgeMalformed`] when no rater produced a usable rating.
    fn assess(&self, subject: &str, goal: &Goal, seeds: &[u64]) -> Result<Assessment> {
        if !goal.is_well_formed() || seeds.is_empty() {
            return Err(Error::InvalidEvaluation);
        }
        let text = prompt(goal, subject);
        let mut qualities = Vec::with_capacity(seeds.len());
        let mut tokens = 0_u64;
        for seed in seeds {
            let mut ratings = vec![];
            let mut failed_calls = 0;
            for rater in 0..self.raters {
                let reply = block_on(self.backend.structured(StructuredRequest {
                    seat: Seat {
                        run_id: format!("seed-{seed}"),
                        role: format!("judge_{rater}"),
                    },
                    model: self.model.clone(),
                    system: SYSTEM.to_owned(),
                    // A Conversant has no seed channel and may ignore the
                    // seat, so the message itself names the draw: otherwise
                    // a seeded backend repeats one rating as N.
                    messages: vec![Message::user(format!(
                        "{text}\n# Rating\nrater {rater}, draw {}\n",
                        super::llm::rater_seed(*seed, rater)
                    ))],
                    tool: TOOL.to_owned(),
                    schema: self.schema(),
                }));
                match reply {
                    Ok(reply) => {
                        tokens = tokens
                            .saturating_add(reply.usage.input_tokens)
                            .saturating_add(reply.usage.output_tokens);
                        ratings.extend(self.parse(&reply.value));
                    }
                    Err(_) => failed_calls += 1,
                }
            }
            if ratings.is_empty() {
                return Err(if failed_calls == self.raters {
                    Error::HostFailed
                } else {
                    Error::JudgeMalformed
                });
            }
            let n = ratings.iter().fold(0.0, |n, _| n + 1.0);
            qualities.push(ratings.iter().sum::<f64>() / n);
        }
        Ok(Assessment {
            eligible: gate(self.calibration.as_ref(), &qualities),
            qualities,
            metric: None,
            rationale: None,
            tokens,
        })
    }
}

impl std::fmt::Debug for BackroomsJudge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackroomsJudge")
            .field("raters", &self.raters)
            .field("model", &self.model)
            .field("novelty", &self.novelty)
            .field("calibrated", &self.calibration.is_some())
            .finish_non_exhaustive()
    }
}
