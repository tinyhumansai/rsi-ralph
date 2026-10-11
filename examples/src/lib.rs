//! Shared plumbing for the end-to-end examples in this package.
//!
//! Each example runs offline by default: model calls are answered by
//! [`Replies`] (canned proposals) or a [`Grader`] (a scripted judge), so the
//! examples need no network or credentials and run in CI. Set
//! `TINYRSI_LIVE_OPENROUTER=1` with `OPENROUTER_API_KEY` to use a real model
//! instead (see [`live_or`]). Every run writes its loop trace to
//! `target/rsi-examples/<example>/<run>/events.jsonl` ([`run_location`]), the
//! directory `apps/rsi-viz` lists with `RSI_RUN_ROOT=target/rsi-examples`.
//!
//! The script-loop subjects are defined once in [`benchmarks`], which both
//! the example binaries and the `benchmarks` runner call; [`harness`] holds
//! the runner's measurement (per-run records, repetition share, aggregation
//! and the comparison table).
//!
//! # Example
//!
//! ```
//! use tinyrsi_examples::{Replies, proposal};
//! let replies = Replies::new([proposal("why", "what", "a.py", "print(1)")]);
//! assert_eq!(replies.remaining(), 1);
//! ```

use std::collections::VecDeque;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tinyrsi::{ModelError, ModelFuture, ModelPort, ModelReply, ModelRequest, RunLocation};

pub mod benchmarks;
pub mod harness;

/// Whether the examples should call a real model.
#[must_use]
pub fn live() -> bool {
    std::env::var("TINYRSI_LIVE_OPENROUTER").as_deref() == Ok("1")
}

/// The `OpenRouter` model named by `OPENROUTER_MODEL` when [`live`], otherwise
/// `offline`.
///
/// # Errors
///
/// Returns an error when live mode is on but the model cannot be configured.
pub fn live_or(offline: Arc<dyn ModelPort>) -> Result<Arc<dyn ModelPort>, Box<dyn Error>> {
    if !live() {
        return Ok(offline);
    }
    let model = default_live_model();
    println!("live model: openrouter {model}");
    live_model(&model)
}

/// The model id `OPENROUTER_MODEL` names, or `openai/gpt-4.1-mini`.
#[must_use]
pub fn default_live_model() -> String {
    std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "openai/gpt-4.1-mini".into())
}

/// An `OpenRouter` model port for the model id `model`, without reading the
/// environment (the API key is read per call).
///
/// # Errors
///
/// Returns an error for an empty or control-containing model id.
pub fn live_model(model: &str) -> Result<Arc<dyn ModelPort>, Box<dyn Error>> {
    Ok(Arc::new(tinyrsi::OpenRouterModel::new(model.to_owned())?))
}

/// A fresh run directory under `target/rsi-examples/<example>/`.
///
/// # Errors
///
/// Returns an error when the clock is set before the Unix epoch.
pub fn run_location(example: &str) -> Result<RunLocation, Box<dyn Error>> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let id = format!("{example}-{stamp}-{}", std::process::id());
    let root = std::env::var("TINYRSI_EXAMPLES_DIR")
        .map_or_else(|_| PathBuf::from("target/rsi-examples"), PathBuf::from);
    Ok(RunLocation {
        dir: root.join(example).join(&id),
        id,
    })
}

/// The real interpreter behind `python3` (pyenv shims need an environment the
/// script loop clears), or `TINYRSI_PYTHON` when set.
///
/// # Errors
///
/// Returns an error when no Python 3 interpreter can be found.
pub fn python() -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(path) = std::env::var("TINYRSI_PYTHON") {
        return Ok(PathBuf::from(path));
    }
    let output = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()?;
    let path = String::from_utf8(output.stdout)?.trim().to_owned();
    if !output.status.success() || path.is_empty() {
        return Err("python3 not found; set TINYRSI_PYTHON to an interpreter".into());
    }
    Ok(PathBuf::from(path))
}

/// The supervisor's `propose_script` tool arguments rewriting one file.
#[must_use]
pub fn proposal(rationale: &str, hypothesis: &str, path: &str, content: &str) -> String {
    serde_json::json!({
        "rationale": rationale,
        "hypothesis": hypothesis,
        "files": [{"path": path, "content": content}],
    })
    .to_string()
}

/// A model port that replays canned replies in order; once one is left it
/// repeats it (a supervisor's repeated proposal is rejected as a duplicate).
#[derive(Debug)]
pub struct Replies(Mutex<VecDeque<String>>);

impl Replies {
    /// Replays `replies` in order.
    #[must_use]
    pub fn new(replies: impl IntoIterator<Item = String>) -> Self {
        Self(Mutex::new(replies.into_iter().collect()))
    }

    /// Replies not yet given (the last one is never used up).
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.0.lock().map_or(0, |q| q.len())
    }
}

impl ModelPort for Replies {
    fn generate(&self, _: ModelRequest) -> ModelFuture {
        let next = self
            .0
            .lock()
            .map(|mut queue| {
                if queue.len() > 1 {
                    queue.pop_front()
                } else {
                    queue.front().cloned()
                }
            })
            .ok()
            .flatten();
        Box::pin(async move {
            let content = next.ok_or(ModelError::CallFailed)?.into_bytes();
            // Nothing was generated, so nothing is charged.
            Ok(ModelReply {
                content,
                input_tokens: 0,
                output_tokens: 0,
            })
        })
    }
}

/// A scripted judge: it answers a `grade` tool call with `grade(prompt)`, a
/// 0-to-10 score and a reason, computed from the prompt it was shown (the
/// goal, the criteria and the subject's text).
pub struct Grader {
    grade: Box<GradeFn>,
}

/// How a [`Grader`] turns a prompt into a 0-to-10 score and a reason.
type GradeFn = dyn Fn(&str) -> (f64, String) + Send + Sync;

impl Grader {
    /// Grades with `grade`.
    #[must_use]
    pub fn new(grade: impl Fn(&str) -> (f64, String) + Send + Sync + 'static) -> Self {
        Self {
            grade: Box::new(grade),
        }
    }
}

impl std::fmt::Debug for Grader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grader").finish_non_exhaustive()
    }
}

impl ModelPort for Grader {
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        let (score, rationale) = (self.grade)(request.prompt());
        let content = serde_json::json!({"score": score.clamp(0.0, 10.0), "rationale": rationale})
            .to_string()
            .into_bytes();
        Box::pin(async move {
            Ok(ModelReply {
                content,
                input_tokens: 0,
                output_tokens: 0,
            })
        })
    }
}

#[cfg(test)]
mod test;
