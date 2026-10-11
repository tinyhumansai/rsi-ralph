//! Both judges against scripted backends: prompts, aggregation, the
//! calibration gate and every failure path.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::{Assessment, Error, Scorer};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tinybackrooms::ScriptedConversant;
use tinyrsi_model::{ModelFuture, ModelPort, ModelReply, ModelRequest};

const SEEDS: [u64; 2] = [3, 4];

fn goal() -> Goal {
    Goal::new("Explain the proof").with_criterion("every step is justified")
}

/// Replies in order and records each request's prompt, seed and
/// reservation, and separately the structured tool it carried.
#[derive(Default)]
struct Scripted {
    replies: Mutex<VecDeque<Result<String, tinyrsi_model::Error>>>,
    seen: Mutex<Vec<(String, u64, u64)>>,
    tools: Mutex<Vec<Option<(String, serde_json::Value)>>>,
}

impl Scripted {
    fn with(replies: Vec<Result<String, tinyrsi_model::Error>>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            ..Self::default()
        })
    }
    fn grades(scores: &[f64]) -> Arc<Self> {
        Self::with(
            scores
                .iter()
                .map(|s| Ok(json!({"score": s, "rationale": format!("gave {s}")}).to_string()))
                .collect(),
        )
    }
}

impl ModelPort for Scripted {
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        self.seen.lock().unwrap().push((
            request.prompt().to_owned(),
            request.seed(),
            request.reservation(),
        ));
        self.tools.lock().unwrap().push(
            request
                .tool()
                .map(|(name, schema)| (name.to_owned(), schema.clone())),
        );
        let next = self.replies.lock().unwrap().pop_front();
        Box::pin(async move {
            let content = next.unwrap_or(Err(tinyrsi_model::Error::CallFailed))?;
            Ok(ModelReply {
                content: content.into_bytes(),
                input_tokens: 10,
                output_tokens: 5,
            })
        })
    }
}

fn passing() -> JudgeCalibration {
    JudgeCalibration::from_labels(
        &[true, false, true, false],
        &[true, false, true, false],
        0.5,
    )
    .unwrap()
}

fn failing() -> JudgeCalibration {
    JudgeCalibration::from_labels(
        &[true, false, true, false],
        &[false, true, true, false],
        0.5,
    )
    .unwrap()
}

#[test]
fn renders_the_goal_criteria_and_subject() {
    let text = prompt(&goal(), "step one\n\n");
    assert_eq!(
        text,
        "# Goal\nExplain the proof\n\n# Criteria\n- every step is justified\n\n# Subject\nstep one\n"
    );
    assert!(!prompt(&Goal::new("g"), "s").contains("# Criteria"));
}

#[test]
fn the_llm_judge_takes_the_median_of_independent_raters_per_seed() {
    let port = Scripted::grades(&[2.0, 9.0, 5.0, 10.0, 6.0, 8.0]);
    let judge = LlmJudge::new(port.clone(), 3, JudgeLimits::default()).unwrap();
    let a: Assessment = judge.assess("proof text", &goal(), &SEEDS).unwrap();
    assert_eq!(a.qualities, vec![0.5, 0.8]);
    assert_eq!(a.tokens, 6 * 15);
    assert_eq!(a.rationale.as_deref(), Some("gave 2"));
    assert!(a.eligible.is_none(), "uncalibrated judges are not eligible");
    let seen = port.seen.lock().unwrap();
    let seeds: Vec<u64> = seen.iter().map(|(_, s, _)| *s).collect();
    let expected: Vec<u64> = [3_u64, 4]
        .iter()
        .flat_map(|seed| (0..3).map(move |rater| rater_seed(*seed, rater)))
        .collect();
    assert_eq!(seeds, expected);
    assert!(seen[0].0.contains("proof text") && seen[0].0.contains("every step"));
    let tools = port.tools.lock().unwrap();
    assert_eq!(tools.len(), 6);
    for tool in tools.iter() {
        let (name, schema) = tool.as_ref().unwrap();
        assert_eq!(name, "grade", "external ports see the forced tool");
        assert_eq!(schema["required"], json!(["score", "rationale"]));
    }
    assert!(format!("{judge:?}").contains("raters: 3"));
}

#[test]
fn an_even_panel_averages_the_middle_grades() {
    let judge = LlmJudge::new(Scripted::grades(&[2.0, 4.0]), 2, JudgeLimits::default()).unwrap();
    let quality = judge.assess("s", &goal(), &[1]).unwrap().qualities[0];
    assert!((quality - 0.3).abs() < 1e-12);
}

#[test]
fn only_a_passing_calibration_makes_the_judge_eligible() {
    let assess = |calibration| {
        let judge = LlmJudge::new(Scripted::grades(&[7.0, 7.0]), 1, JudgeLimits::default())
            .unwrap()
            .calibrated(calibration);
        judge.assess("s", &goal(), &SEEDS).unwrap().eligible
    };
    assert_eq!(assess(passing()), Some(vec![0.7, 0.7]));
    assert_eq!(assess(failing()), None);
}

#[test]
fn the_llm_judge_refuses_malformed_replies() {
    for reply in [
        "not json".to_owned(),
        json!({"score": 11, "rationale": "r"}).to_string(),
        json!({"score": 5}).to_string(),
        json!({"rationale": "r"}).to_string(),
    ] {
        let judge =
            LlmJudge::new(Scripted::with(vec![Ok(reply)]), 1, JudgeLimits::default()).unwrap();
        assert_eq!(judge.assess("s", &goal(), &[1]), Err(Error::JudgeMalformed));
    }
}

#[test]
fn the_llm_judge_surfaces_model_failures_and_bad_inputs() {
    let judge = LlmJudge::new(Scripted::with(vec![]), 1, JudgeLimits::default()).unwrap();
    assert_eq!(
        judge.assess("s", &goal(), &[1]),
        Err(Error::Model(tinyrsi_model::Error::CallFailed))
    );
    assert_eq!(
        judge.assess("s", &Goal::new(" "), &[1]),
        Err(Error::InvalidEvaluation)
    );
    assert_eq!(
        judge.assess("s", &goal(), &[]),
        Err(Error::InvalidEvaluation)
    );
    let tiny = JudgeLimits {
        call_tokens: 10,
        ..JudgeLimits::default()
    };
    let judge = LlmJudge::new(Scripted::grades(&[5.0]), 1, tiny).unwrap();
    assert_eq!(
        judge.assess("s", &goal(), &[1]),
        Err(Error::Model(tinyrsi_model::Error::InvalidRequest))
    );
    for raters in [0, 256] {
        assert!(LlmJudge::new(Scripted::grades(&[]), raters, JudgeLimits::default()).is_err());
    }
}

/// A tinybackrooms backend answering each rater from `rate(role)`.
fn backend(
    rate: impl Fn(&str) -> tinybackrooms::Result<serde_json::Value> + Send + Sync + 'static,
) -> Arc<ScriptedConversant> {
    Arc::new(ScriptedConversant::new(
        |_| Ok(String::new()),
        move |call| {
            assert_eq!(call.tool, "rate_subject");
            assert!(call.messages[0].content.contains("# Subject"));
            rate(&call.seat.role)
        },
    ))
}

#[test]
fn the_backrooms_judge_averages_ratings_on_the_one_to_seven_scale() {
    let judge = BackroomsJudge::new(
        backend(|role| {
            Ok(match role {
                "judge_0" => json!({"goal_fit": 7}),
                "judge_1" => json!({"goal_fit": 4}),
                _ => json!({"goal_fit": 1}),
            })
        }),
        3,
    )
    .unwrap()
    .with_model("judge-model");
    let a = judge.assess("s", &goal(), &SEEDS).unwrap();
    assert_eq!(a.qualities, vec![0.5, 0.5]);
    assert!(a.eligible.is_none());
    assert!(format!("{judge:?}").contains("judge-model"));
}

#[test]
fn novelty_mode_multiplies_fit_by_novelty() {
    let judge = BackroomsJudge::new(backend(|_| Ok(json!({"goal_fit": 7, "novelty": 4}))), 1)
        .unwrap()
        .with_novelty()
        .calibrated(passing());
    let a = judge.assess("s", &goal(), &[1]).unwrap();
    assert_eq!(a.qualities, vec![0.5]);
    assert_eq!(a.eligible, Some(vec![0.5]));
    let missing = BackroomsJudge::new(backend(|_| Ok(json!({"goal_fit": 7}))), 1)
        .unwrap()
        .with_novelty();
    assert_eq!(
        missing.assess("s", &goal(), &[1]),
        Err(Error::JudgeMalformed)
    );
}

#[test]
fn the_backrooms_judge_skips_failed_raters_and_reports_a_dead_panel() {
    let partial = BackroomsJudge::new(
        backend(|role| {
            if role == "judge_0" {
                Err(tinybackrooms::Error::Conversant("down".into()))
            } else {
                Ok(json!({"goal_fit": 7}))
            }
        }),
        2,
    )
    .unwrap();
    assert_eq!(
        partial.assess("s", &goal(), &[1]).unwrap().qualities,
        vec![1.0]
    );
    let dead = BackroomsJudge::new(
        backend(|_| Err(tinybackrooms::Error::Conversant("down".into()))),
        2,
    )
    .unwrap();
    assert_eq!(dead.assess("s", &goal(), &[1]), Err(Error::HostFailed));
    let garbled = BackroomsJudge::new(backend(|_| Ok(json!({"goal_fit": 9}))), 2).unwrap();
    assert_eq!(
        garbled.assess("s", &goal(), &[1]),
        Err(Error::JudgeMalformed)
    );
    assert_eq!(
        garbled.assess("s", &goal(), &[]),
        Err(Error::InvalidEvaluation)
    );
    assert!(BackroomsJudge::new(backend(|_| Ok(json!({}))), 0).is_err());
}

#[test]
fn rater_seeds_stay_distinct_for_seeds_that_differ_only_in_high_bits() {
    let port = Scripted::grades(&[5.0, 5.0, 5.0, 5.0]);
    let judge = LlmJudge::new(port.clone(), 2, JudgeLimits::default()).unwrap();
    judge.assess("s", &goal(), &[0, 1 << 56]).unwrap();
    let seeds: std::collections::BTreeSet<u64> = port
        .seen
        .lock()
        .unwrap()
        .iter()
        .map(|(_, s, _)| *s)
        .collect();
    assert_eq!(
        seeds.len(),
        4,
        "every (seed, rater) pair gets its own model seed"
    );
}

#[test]
fn every_backrooms_rating_sends_a_distinct_model_request() {
    // ModelConversant ignores the seat and always sends its own seed, so the
    // only thing that can tell ratings apart is the message itself.
    let rating = || Ok(json!({"goal_fit": 4}).to_string());
    let port = Scripted::with((0..4).map(|_| rating()).collect());
    let conversant =
        tinyrsi_model::ModelConversant::new(port.clone(), 7, 1 << 20, 1 << 16, 256, 1000).unwrap();
    let judge = BackroomsJudge::new(Arc::new(conversant), 2).unwrap();
    judge.assess("s", &goal(), &SEEDS).unwrap();
    let requests: std::collections::BTreeSet<(String, u64)> = port
        .seen
        .lock()
        .unwrap()
        .iter()
        .map(|(prompt, seed, _)| (prompt.clone(), *seed))
        .collect();
    assert_eq!(
        requests.len(),
        4,
        "every (seed, rater) pair is its own request"
    );
}

#[test]
fn the_backrooms_judge_rejects_fractional_ratings() {
    let mixed = BackroomsJudge::new(
        backend(|role| {
            Ok(if role == "judge_0" {
                json!({"goal_fit": 4.5})
            } else {
                json!({"goal_fit": 7})
            })
        }),
        2,
    )
    .unwrap();
    assert_eq!(
        mixed.assess("s", &goal(), &[1]).unwrap().qualities,
        vec![1.0],
        "a fractional rating is skipped like any other unusable one"
    );
    let fractional = BackroomsJudge::new(backend(|_| Ok(json!({"goal_fit": 4.5}))), 2).unwrap();
    assert_eq!(
        fractional.assess("s", &goal(), &[1]),
        Err(Error::JudgeMalformed)
    );
    let novelty = BackroomsJudge::new(backend(|_| Ok(json!({"goal_fit": 7, "novelty": 2.5}))), 1)
        .unwrap()
        .with_novelty();
    assert_eq!(
        novelty.assess("s", &goal(), &[1]),
        Err(Error::JudgeMalformed)
    );
}
