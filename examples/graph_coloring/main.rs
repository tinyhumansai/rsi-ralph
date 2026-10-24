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
//!
//! The subject, scorer, policy and canned replies are defined once in
//! `rsi_ralph_examples::benchmarks`, which the `benchmarks` runner shares.
use std::{error::Error, sync::Arc};
use rsi_ralph::{ModelPort, StopReason};
use rsi_ralph_examples::{
    benchmarks::{Benchmark, Models},
    live_or, python, run_location,
};

fn main() -> Result<(), Box<dyn Error>> {
    let benchmark = Benchmark::GraphColoring;
    let run = run_location(benchmark.name())?;
    let models = Models {
        supervisor: live_or(Arc::new(benchmark.offline_supervisor()))?,
        judge: benchmark
            .offline_judge()
            .map(|grader| live_or(Arc::new(grader) as Arc<dyn ModelPort>))
            .transpose()?,
    };
    let report = benchmark.run(&run, &python()?, models)?;
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
    if !rsi_ralph_examples::live() && report.reason != StopReason::GoalReached {
        return Err(format!(
            "expected the goal to be reached, stopped: {:?}",
            report.reason
        )
        .into());
    }
    Ok(())
}
