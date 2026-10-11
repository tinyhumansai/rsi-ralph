//! Solve a graph theory problem: colour graphs with as few colours as possible.
//!
//! The subject is `coloring.py`, whose `color(n, edges)` starts as first fit
//! in label order (16 to 18 colours). The fixed checker `check_coloring.py`
//! builds, per seed, a 150-vertex random graph with a hidden 6-colouring,
//! rejects any colouring with a conflicting edge, and prints the number of
//! colours used. The loop evolves `coloring.py` until every seed is coloured
//! with 6, the optimum. Validity is a hard gate: an invalid colouring prints
//! no metric, so its seed fails and the candidate is rejected.
//!
//! The score is the measured colour count alone (`MetricScorer`). There is no
//! judge: the checker already decides correctness.
//!
//! Offline (the default) the supervisor replays three canned rewrites: DSATUR
//! (better), a shuffled first fit with a bug (invalid colourings, rejected),
//! and DSATUR followed by `TabuCol` (reaches 6). With
//! `TINYRSI_LIVE_OPENROUTER=1` the supervisor is a real model. The run's
//! `events.jsonl` lands under `target/rsi-examples/graph_coloring/`.
use std::{error::Error, sync::Arc};
use tinyrsi::{
    JsonlTrace, LoopPolicy, MetricDirection, MetricScorer, MetricSpec, MonotonicClock, Projected,
    ScriptCandidate, ScriptCommand, ScriptSubject, ScriptSupervisor, StopReason, SupervisorLimits,
    run_script_loop_with,
};
use tinyrsi_examples::{Replies, live_or, proposal, python, run_location};

const CHECKER: &str = include_str!("check_coloring.py");
const START: &str = include_str!("coloring.py");
const DSATUR: &str = include_str!("proposals/dsatur.py");
const RANDOM_ORDER: &str = include_str!("proposals/random_order.py");
const TABUCOL: &str = include_str!("proposals/tabucol.py");

const BRIEF: &str = "coloring.py defines color(n, edges): given n vertices labelled 0..n-1 and a \
list of undirected edges (a, b), return a list of n non-negative integers so that no edge joins \
two vertices of the same colour. The fixed checker check_coloring.py feeds it random graphs with \
150 vertices and about 3,300 edges, rejects any invalid colouring (the seed then fails), and \
prints the number of distinct colours used. Rewrite coloring.py so color() uses as few colours \
as possible. Use only the Python standard library; stay well under 20 seconds per graph; do not \
read files, the environment, or the checker. Name the algorithm in a docstring.";

fn canned() -> Replies {
    Replies::new([
        proposal(
            "First fit in label order ignores structure; constrained vertices get colours late.",
            "DSATUR colours the most saturated vertex first and needs fewer colours.",
            "coloring.py",
            DSATUR,
        ),
        proposal(
            "Label order is arbitrary, so a shuffled order should be no worse.",
            "First fit in a random order matches DSATUR at lower cost.",
            "coloring.py",
            RANDOM_ORDER,
        ),
        proposal(
            "DSATUR is greedy and stops at its first valid colouring.",
            "TabuCol local search shrinks the DSATUR palette to the optimum.",
            "coloring.py",
            TABUCOL,
        ),
    ])
}

fn main() -> Result<(), Box<dyn Error>> {
    let run = run_location("graph_coloring")?;
    let workspace = run.dir.join("workspace");
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(workspace.join("check_coloring.py"), CHECKER)?;
    std::fs::write(workspace.join("coloring.py"), START)?;
    let subject = ScriptSubject {
        name: "coloring.py".into(),
        brief: BRIEF.into(),
        workspace,
        editable: vec!["coloring.py".into()],
        command: ScriptCommand {
            program: python()?,
            args: vec!["check_coloring.py".into()],
        },
        metric: MetricSpec {
            name: "colors".into(),
            direction: MetricDirection::Minimize,
            target: Some(6.0),
        },
        seeds: vec![1, 2, 3],
        timeout_ms: 30_000,
        output_cap: 65_536,
        env: vec![("PYTHONDONTWRITEBYTECODE".into(), "1".into())],
    };
    let supervisor =
        ScriptSupervisor::new(live_or(Arc::new(canned()))?, SupervisorLimits::default())?;
    let scorer = Projected::new(
        |c: &ScriptCandidate| c.metrics.as_slice(),
        MetricScorer::new(MetricDirection::Minimize),
    );
    let policy = LoopPolicy {
        max_epochs: 6,
        proposals_per_epoch: 2,
        patience: 3,
        ..LoopPolicy::default()
    };
    let mut trace = JsonlTrace::create(run.dir.join("events.jsonl"), MonotonicClock::new())?;
    let report = run_script_loop_with(&subject, &supervisor, &scorer, &policy, &run, &mut trace)?;
    println!(
        "{:?} after {} epochs: {:.2} -> {:.2} colours (optimum 6); {} promotions, {} rejected",
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
