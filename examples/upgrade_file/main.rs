//! Upgrade a file to a written standard.
//!
//! The subject is `slug.py`, a working but untidy `slugify` function. The
//! team's standard is written down as a goal (docstrings, type annotations,
//! no bare `except:`, short lines). The fixed checker `check_standard.py`:
//!
//! - first runs behavior tests, so a rewrite that changes behavior fails its
//!   seeds and is rejected however tidy it looks;
//! - then counts violations of the standard.
//!
//! The loop evolves `slug.py` to zero violations, scoring each candidate with
//! a composite of the measured violations (`MetricScorer`, 60%) and an
//! `LlmJudge` (40%) that reads the source against the standard. Only the
//! measured evidence can carry an accept, because the judge is uncalibrated.
//!
//! Offline (the default) the supervisor replays three canned rewrites (add
//! docstrings, a regex rewrite that breaks Unicode slugs, the full upgrade),
//! and the judge is a scripted grader. With `TINYRSI_LIVE_OPENROUTER=1`
//! both are a real model. The trace lands under
//! `target/rsi-examples/upgrade_file/`.
use std::{error::Error, path::Path, sync::Arc};
use tinyrsi::{
    Composite, Goal, JsonlTrace, JudgeLimits, LlmJudge, LoopPolicy, MetricDirection, MetricScorer,
    MetricSpec, MonotonicClock, Projected, ScriptCandidate, ScriptCommand, ScriptSubject,
    ScriptSupervisor, StopReason, SupervisorLimits, run_script_loop_with,
};
use tinyrsi_examples::{Grader, Replies, live_or, proposal, python, run_location};

const CHECKER: &str = include_str!("check_standard.py");
const START: &str = include_str!("slug.py");
const DOCSTRINGS: &str = include_str!("proposals/docstrings.py");
const REGEX: &str = include_str!("proposals/regex.py");
const STANDARD: &str = include_str!("proposals/standard.py");

/// The written standard, as the judge and the supervisor read it.
fn standard() -> Goal {
    Goal::new("Bring slug.py up to the team's Python standard without changing behavior.")
        .with_criterion("the module and every function have a docstring")
        .with_criterion("every parameter and return value is type-annotated")
        .with_criterion("no bare except clauses")
        .with_criterion("no line is longer than 79 characters")
        .with_criterion("the code is readable without the original author")
}

fn brief(goal: &Goal) -> String {
    let mut text = format!(
        "{} The fixed checker check_standard.py runs behavior tests (slugify must keep \
         its exact output, including non-ASCII letters) and prints the number of standard \
         violations. Criteria:",
        goal.statement
    );
    for criterion in &goal.criteria {
        text.push_str("\n- ");
        text.push_str(criterion);
    }
    text
}

fn canned() -> Replies {
    Replies::new([
        proposal(
            "The module and slugify are undocumented.",
            "Docstrings remove two violations.",
            "slug.py",
            DOCSTRINGS,
        ),
        proposal(
            "A regex is shorter and fully annotated.",
            "One substitution satisfies the standard.",
            "slug.py",
            REGEX,
        ),
        proposal(
            "Annotate, drop the bare except, and build the slug from word runs.",
            "Every remaining violation goes, with behavior unchanged.",
            "slug.py",
            STANDARD,
        ),
    ])
}

/// The offline judge: one point per criterion it can see met in the source.
fn grader() -> Grader {
    Grader::new(|prompt| {
        let source = prompt.split("# Subject").nth(1).unwrap_or_default();
        let checks = [
            source.trim_start().starts_with("\"\"\""),
            source.contains("->"),
            !source.contains("except:"),
            source.lines().all(|line| line.len() <= 79),
            source.matches("\"\"\"").count() >= 4,
        ];
        let met = checks.iter().filter(|ok| **ok).count();
        (
            f64::from(u8::try_from(met * 2).unwrap_or(10)),
            format!("{met} of 5 criteria visibly met"),
        )
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let goal = standard();
    let run = run_location("upgrade_file")?;
    let workspace = run.dir.join("workspace");
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(workspace.join("check_standard.py"), CHECKER)?;
    std::fs::write(workspace.join("slug.py"), START)?;
    let subject = ScriptSubject {
        name: "slug.py".into(),
        brief: brief(&goal),
        workspace,
        editable: vec!["slug.py".into()],
        command: ScriptCommand {
            program: python()?,
            args: vec!["check_standard.py".into()],
        },
        metric: MetricSpec {
            name: "violations".into(),
            direction: MetricDirection::Minimize,
            target: Some(0.0),
        },
        seeds: vec![1, 2],
        timeout_ms: 30_000,
        output_cap: 65_536,
        env: vec![("PYTHONDONTWRITEBYTECODE".into(), "1".into())],
    };
    let supervisor =
        ScriptSupervisor::new(live_or(Arc::new(canned()))?, SupervisorLimits::default())?;
    let judge = LlmJudge::new(live_or(Arc::new(grader()))?, 1, JudgeLimits::default())?;
    let scorer = Composite::new()
        .with(
            0.6,
            Projected::new(
                |c: &ScriptCandidate| c.metrics.as_slice(),
                MetricScorer::new(MetricDirection::Minimize),
            ),
        )
        .with(
            0.4,
            Projected::new(
                |c: &ScriptCandidate| {
                    c.sources
                        .get(Path::new("slug.py"))
                        .map_or("", String::as_str)
                },
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
        "{:?} after {} epochs: {} -> {} standard violations; {} promotions, {} rejected",
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
