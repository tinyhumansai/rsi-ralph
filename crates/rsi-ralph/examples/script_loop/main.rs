//! Improve a curve-fitting script with the `RSI Ralph` script loop.
//!
//! Offline (the default) a canned supervisor replays three prepared model
//! rewrites and one path-escape attempt, so the loop runs without a network or
//! credentials. With `TINYRSI_LIVE_OPENROUTER=1` and `OPENROUTER_API_KEY` set,
//! an `OpenRouter` model supervises instead. See this directory's `README.md`.
use std::{
    collections::VecDeque,
    error::Error,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use rsi_ralph::{
    JsonlTrace, LoopPolicy, MetricDirection, MetricSpec, ModelFuture, ModelPort, ModelReply,
    ModelRequest, MonotonicClock, OpenRouterModel, RunLocation, ScriptCommand, ScriptSubject,
    ScriptSupervisor, SupervisorLimits, run_script_loop,
};

const HARNESS: &str = include_str!("curve_fit.py");
const MODEL: &str = include_str!("model.py");
const CUBIC: &str = include_str!("proposals/cubic.py");
const CONSTANT: &str = include_str!("proposals/constant.py");
const DAMPED: &str = include_str!("proposals/damped.py");

/// The supervisor's brief, shared with the `curve_fit` benchmark in
/// `rsi-ralph-examples` (trailing newline trimmed where it is used).
const BRIEF: &str = include_str!("brief.txt");

/// Replays canned tool-call arguments; once they run out it repeats the last,
/// which the loop rejects as a duplicate.
struct Canned(Mutex<VecDeque<String>>);

impl ModelPort for Canned {
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
            let content = next.ok_or(rsi_ralph::ModelError::CallFailed)?.into_bytes();
            // Nothing was generated, so nothing is charged.
            Ok(ModelReply {
                content,
                input_tokens: 0,
                output_tokens: 0,
            })
        })
    }
}

fn proposal(
    approach: &str,
    rationale: &str,
    hypothesis: &str,
    path: &str,
    content: &str,
) -> String {
    serde_json::json!({
        "approach": approach,
        "rationale": rationale,
        "hypothesis": hypothesis,
        "files": [{"path": path, "content": content}],
    })
    .to_string()
}

fn canned() -> Canned {
    Canned(Mutex::new(VecDeque::from([
        proposal(
            "cubic least squares",
            "The straight line is undertrained and cannot bend with the signal.",
            "An exact cubic least-squares fit lowers held-out RMSE.",
            "model.py",
            CUBIC,
        ),
        proposal(
            "constant predictor",
            "Noise dominates; a constant is the most robust predictor.",
            "Predicting the training peak lowers RMSE.",
            "model.py",
            CONSTANT,
        ),
        proposal(
            "evaluate on training data",
            "Change the harness to evaluate on training points instead.",
            "Scoring on training data lowers RMSE.",
            "../curve_fit.py",
            HARNESS,
        ),
        proposal(
            "damped sinusoid fit",
            "The residuals oscillate with a decaying envelope: fit that family.",
            "A damped sinusoid fit reaches the noise floor of about 0.05.",
            "model.py",
            DAMPED,
        ),
    ])))
}

/// Settings from `--flag value` arguments, falling back to environment.
fn setting(args: &[String], flag: &str, var: &str, default: &str) -> String {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
        .or_else(|| std::env::var(var).ok())
        .unwrap_or_else(|| default.to_owned())
}

/// The real interpreter behind `python3` (pyenv shims need an environment the
/// loop clears), or `TINYRSI_PYTHON` when set.
fn python() -> Result<PathBuf, Box<dyn Error>> {
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

fn workspace(run: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let dir = run.join("workspace");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("curve_fit.py"), HARNESS)?;
    std::fs::write(dir.join("model.py"), MODEL)?;
    Ok(dir)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let live = std::env::var("TINYRSI_LIVE_OPENROUTER").as_deref() == Ok("1");
    let epochs = setting(&args, "--epochs", "TINYRSI_SCRIPT_LOOP_EPOCHS", "4").parse()?;
    let proposals = setting(&args, "--proposals", "TINYRSI_SCRIPT_LOOP_PROPOSALS", "2").parse()?;
    let target: f64 = setting(&args, "--target", "TINYRSI_SCRIPT_LOOP_TARGET", "0.06").parse()?;
    let budget = setting(&args, "--budget", "TINYRSI_SCRIPT_LOOP_BUDGET", "400000").parse()?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let id = format!("curve-fit-{stamp}-{}", std::process::id());
    let root = std::env::var("TINYRSI_SCRIPT_LOOP_DIR")
        .map_or_else(|_| PathBuf::from("target/rsi-script-loop"), PathBuf::from);
    let run = RunLocation {
        id: id.clone(),
        dir: root.join(&id),
    };
    let subject = ScriptSubject {
        name: "curve_fit.py".into(),
        brief: BRIEF.trim_end().into(),
        workspace: workspace(&run.dir)?,
        editable: vec!["model.py".into()],
        command: ScriptCommand {
            program: python()?,
            args: vec!["curve_fit.py".into()],
        },
        metric: MetricSpec {
            name: "rmse".into(),
            direction: MetricDirection::Minimize,
            target: Some(target),
        },
        seeds: vec![1, 2, 3],
        timeout_ms: 60_000,
        output_cap: 65_536,
        env: vec![("PYTHONDONTWRITEBYTECODE".into(), "1".into())],
    };
    let port: Arc<dyn ModelPort> = if live {
        let model =
            std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "openai/gpt-4.1-mini".into());
        println!("live supervisor: openrouter {model}");
        Arc::new(OpenRouterModel::new(model)?)
    } else {
        println!("offline supervisor: canned proposals (set TINYRSI_LIVE_OPENROUTER=1 for live)");
        Arc::new(canned())
    };
    let supervisor = ScriptSupervisor::new(port, SupervisorLimits::default())?;
    let policy = LoopPolicy {
        max_epochs: epochs,
        proposals_per_epoch: proposals,
        patience: 3,
        token_budget: budget,
        ..LoopPolicy::default()
    };
    let mut trace = JsonlTrace::create(run.dir.join("events.jsonl"), MonotonicClock::new())?;
    let report = run_script_loop(&subject, &supervisor, &policy, &run, &mut trace)?;
    println!(
        "{:?} after {} epochs: rmse {:.4} -> {:.4} (target {target}); {} promotions, {} rejected, {} tokens",
        report.reason,
        report.epochs,
        report.baseline_metric,
        report.incumbent_metric,
        report.promotions,
        report.rejected,
        report.tokens
    );
    println!("run directory: {}", run.dir.display());
    Ok(())
}
