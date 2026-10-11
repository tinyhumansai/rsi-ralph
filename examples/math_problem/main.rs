//! Solve a math problem: compute pi to double precision.
//!
//! The subject is `pi.py`, whose `estimate()` starts as a 1,000-term Leibniz
//! series (about 3 correct digits). The fixed checker `check_pi.py` prints how
//! many digits are right. The loop evolves `pi.py` until it reaches 12
//! digits, scoring each candidate with a composite of:
//!
//! - the measured digits (`MetricScorer`, 70%), the only accept-eligible
//!   evidence; and
//! - an `LlmJudge` (30%) reading the source against the goal's criteria (is
//!   the method named and each step justified?). It is uncalibrated, so it
//!   can widen a margin but never accept a candidate on its own.
//!
//! Offline (the default) the supervisor replays three canned rewrites (more
//! terms, a slower product, Machin's formula) and the judge is a scripted
//! grader. With `TINYRSI_LIVE_OPENROUTER=1` both are a real model. The run's
//! `events.jsonl` lands under `target/rsi-examples/math_problem/`.
use std::{error::Error, path::Path, sync::Arc};
use tinyrsi::{
    Composite, JsonlTrace, JudgeLimits, LlmJudge, LoopPolicy, MetricDirection, MetricScorer,
    MetricSpec, MonotonicClock, Projected, ScriptCandidate, ScriptCommand, ScriptSubject,
    ScriptSupervisor, StopReason, SupervisorLimits, run_script_loop_with,
};
use tinyrsi_examples::{Grader, Replies, live_or, proposal, python, run_location, with_approach};

const CHECKER: &str = include_str!("check_pi.py");
const START: &str = include_str!("pi.py");
const MORE_TERMS: &str = include_str!("proposals/more_terms.py");
const WALLIS: &str = include_str!("proposals/wallis.py");
const MACHIN: &str = include_str!("proposals/machin.py");

const BRIEF: &str = "pi.py defines estimate(), which must return pi as a float. The fixed \
checker check_pi.py prints how many decimal digits are correct. Rewrite pi.py so estimate() \
is correct to at least 12 digits, using only the Python standard library and no stored \
constant (math.pi is not allowed). Name the method in a docstring and justify each step.";

fn canned() -> Replies {
    Replies::new([
        with_approach(
            "Leibniz series",
            proposal(
                "The Leibniz series converges slowly; 1,000 terms leave an error near 1e-3.",
                "200,000 terms gain about two more digits.",
                "pi.py",
                MORE_TERMS,
            ),
        ),
        with_approach(
            "Wallis product",
            proposal(
                "A product converges differently from a series; try Wallis.",
                "The Wallis product reaches more digits than the series.",
                "pi.py",
                WALLIS,
            ),
        ),
        with_approach(
            "Machin arctan formula",
            proposal(
                "Machin's arctan identity converges geometrically.",
                "Twenty arctan terms reach full double precision.",
                "pi.py",
                MACHIN,
            ),
        ),
    ])
}

/// The offline judge: it rewards a named, justified method.
fn grader() -> Grader {
    Grader::new(|prompt| {
        let subject = prompt.split("# Subject").nth(1).unwrap_or_default();
        let named = ["Leibniz", "Wallis", "Machin"]
            .iter()
            .any(|m| subject.contains(m));
        let justified = subject.matches("\"\"\"").count() >= 4 || subject.contains("identity");
        match (named, justified) {
            (true, true) => (9.0, "names its method and justifies each step".into()),
            (true, false) => (6.0, "names its method but justifies little".into()),
            _ => (2.0, "no method named, no justification".into()),
        }
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let run = run_location("math_problem")?;
    let workspace = run.dir.join("workspace");
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(workspace.join("check_pi.py"), CHECKER)?;
    std::fs::write(workspace.join("pi.py"), START)?;
    let subject = ScriptSubject {
        name: "pi.py".into(),
        brief: BRIEF.into(),
        workspace,
        editable: vec!["pi.py".into()],
        command: ScriptCommand {
            program: python()?,
            args: vec!["check_pi.py".into()],
        },
        metric: MetricSpec {
            name: "digits".into(),
            direction: MetricDirection::Maximize,
            target: Some(12.0),
        },
        seeds: vec![1, 2, 3],
        timeout_ms: 30_000,
        output_cap: 65_536,
        env: vec![("PYTHONDONTWRITEBYTECODE".into(), "1".into())],
    };
    let supervisor =
        ScriptSupervisor::new(live_or(Arc::new(canned()))?, SupervisorLimits::default())?;
    let judge = LlmJudge::new(live_or(Arc::new(grader()))?, 1, JudgeLimits::default())?;
    let scorer = Composite::new()
        .with(
            0.7,
            Projected::new(
                |c: &ScriptCandidate| c.metrics.as_slice(),
                MetricScorer::new(MetricDirection::Maximize),
            ),
        )
        .with(
            0.3,
            Projected::new(
                |c: &ScriptCandidate| c.sources.get(Path::new("pi.py")).map_or("", String::as_str),
                judge,
            ),
        );
    let policy = LoopPolicy {
        max_epochs: 4,
        proposals_per_epoch: 1,
        patience: 3,
        ..LoopPolicy::default()
    };
    let mut trace = JsonlTrace::create(run.dir.join("events.jsonl"), MonotonicClock::new())?;
    let report = run_script_loop_with(&subject, &supervisor, &scorer, &policy, &run, &mut trace)?;
    println!(
        "{:?} after {} epochs: {:.1} -> {:.1} correct digits of pi; {} promotions, {} rejected",
        report.reason,
        report.epochs,
        report.baseline_metric,
        report.incumbent_metric,
        report.promotions,
        report.rejected,
    );
    println!("trace: {}", run.dir.join("events.jsonl").display());
    // Offline, the canned run must reach its goal; CI runs this example.
    if !tinyrsi_examples::live() && report.reason != StopReason::GoalReached {
        return Err(format!(
            "expected the goal to be reached, stopped: {:?}",
            report.reason
        )
        .into());
    }
    Ok(())
}
