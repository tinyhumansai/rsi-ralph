//! Run the script-loop benchmarks as a matrix and compare the results.
//!
//! ```sh
//! cargo run -p tinyrsi-examples --example benchmarks -- \
//!   [--benchmarks a,b] [--models m1,m2] [--repeats N] [--jobs N] [--out DIR]
//! ```
//!
//! Every benchmark × model × repeat is one independent run of the script
//! loop, written to `<out>/<benchmark>/<model>/<repeat>/` (`events.jsonl`,
//! `ledger.json`). The runner then writes `summary.json` and `summary.md` to
//! `<out>` (default `target/rsi-benchmarks/<unix-seconds>/`) and prints the
//! comparison table: goal rate, median epochs to goal, median tokens, median
//! final metric, mean repetition share and median wall time.
//!
//! Offline (the default) each benchmark runs on its canned supervisor and
//! scripted judge under the model label `offline`, and the runner fails if
//! any run misses its goal, so CI can run it. With
//! `TINYRSI_LIVE_OPENROUTER=1` and `OPENROUTER_API_KEY`, each `--models` id
//! (default `OPENROUTER_MODEL`, else `openai/gpt-4.1-mini`) supervises and
//! judges. Definitions: `docs/specs/loop-benchmarks.md`.
use std::{
    error::Error,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tinyrsi_examples::harness::{
    Summary, aggregate,
    options::{Options, USAGE},
    render_markdown,
    runner::{ModelChoice, execute, plan, write_summary},
};
use tinyrsi_examples::{default_live_model, live, python};

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::parse(std::env::args().skip(1)).map_err(|e| format!("{e}\n{USAGE}"))?;
    let models = if live() {
        options
            .models
            .clone()
            .unwrap_or_else(|| vec![default_live_model()])
            .into_iter()
            .map(ModelChoice::Live)
            .collect()
    } else {
        if options.models.is_some() {
            println!("offline: --models ignored (set TINYRSI_LIVE_OPENROUTER=1 for live models)");
        }
        vec![ModelChoice::Offline]
    };
    let out = if let Some(out) = &options.out {
        out.clone()
    } else {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        PathBuf::from("target/rsi-benchmarks").join(stamp.to_string())
    };
    let jobs = plan(&options.benchmarks, &models, options.repeats, &out);
    println!("{} runs under {}", jobs.len(), out.display());
    let records = execute(jobs, &python()?, options.jobs, &|r| {
        println!(
            "{} {} #{}: {:?} after {} epochs, {} tokens, {:.1} s{}",
            r.benchmark,
            r.model,
            r.repeat,
            r.stop,
            r.epochs,
            r.tokens,
            seconds(r.wall_ms),
            r.error
                .as_deref()
                .map_or_else(String::new, |e| format!(" (error: {e})")),
        );
    });
    let header = format!(
        "# Loop benchmarks\n\n- Mode: {}\n- Benchmarks: {}\n- Models: {}\n- Repeats: {}",
        if live() { "live" } else { "offline" },
        options
            .benchmarks
            .iter()
            .map(|b| b.name())
            .collect::<Vec<_>>()
            .join(", "),
        models
            .iter()
            .map(ModelChoice::label)
            .collect::<Vec<_>>()
            .join(", "),
        options.repeats,
    );
    let summary = Summary {
        rows: aggregate(&records),
        runs: records,
    };
    let (json, markdown) = write_summary(&out, &summary, &header)?;
    println!("\n{}", render_markdown(&summary.rows));
    println!(
        "summary: {}\n         {}",
        json.display(),
        markdown.display()
    );
    // Offline, every canned run must reach its goal; CI runs this example.
    let missed = summary.runs.iter().filter(|r| !r.goal_reached).count();
    if !live() && missed > 0 {
        return Err(format!("{missed} offline runs missed their goal").into());
    }
    Ok(())
}

/// Milliseconds as seconds, for display.
#[expect(
    clippy::cast_precision_loss,
    reason = "wall times stay far below 2^53 ms"
)]
fn seconds(ms: u64) -> f64 {
    ms as f64 / 1000.0
}
