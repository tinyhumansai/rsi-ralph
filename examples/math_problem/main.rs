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
    let benchmark = Benchmark::MathProblem;
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
    if !rsi_ralph_examples::live() && report.reason != StopReason::GoalReached {
        return Err(format!(
            "expected the goal to be reached, stopped: {:?}",
            report.reason
        )
        .into());
    }
    Ok(())
}
