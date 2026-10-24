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
//!
//! The subject, scorer, policy and canned replies are defined once in
//! `rsi_ralph_examples::benchmarks`, which the `benchmarks` runner shares.
use rsi_ralph::{ModelPort, StopReason};
use rsi_ralph_examples::{
    benchmarks::{Benchmark, Models},
    live_or, python, run_location,
};
use std::{error::Error, sync::Arc};

fn main() -> Result<(), Box<dyn Error>> {
    let benchmark = Benchmark::UpgradeFile;
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
    if !rsi_ralph_examples::live() && report.reason != StopReason::GoalReached {
        return Err(format!(
            "expected the goal to be reached, stopped: {:?}",
            report.reason
        )
        .into());
    }
    Ok(())
}
