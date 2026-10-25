//! A score history as plain text for a terminal, or as CSV with one row per
//! measured checkout.

use std::fmt::Write;

use super::{RunSummary, ScoreHistory, ScorePoint};

fn score(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), |v| format!("{v:.4}"))
}

#[expect(
    clippy::cast_precision_loss,
    reason = "minutes are shown to one decimal"
)]
fn minutes(ms: Option<u64>) -> String {
    ms.map_or_else(
        || "-".to_owned(),
        |ms| format!("{:.1} min", ms as f64 / 60_000.0),
    )
}

fn brief(s: &RunSummary) -> String {
    format!(
        "run {} (start {}, best {}, end {}, {} promoted of {})",
        s.run,
        score(s.start),
        score(s.best),
        score(s.end),
        s.promoted,
        s.candidates
    )
}

/// The history as text: each run with its baseline, each epoch with its
/// timing and candidates, then each upgrade with the runs around it.
#[must_use]
pub fn text(history: &ScoreHistory) -> String {
    let mut out = format!("score history: {}\n", history.r#loop);
    if history.runs.is_empty() {
        out.push_str("no stored runs\n");
    }
    for run in &history.runs {
        let _ = writeln!(
            out,
            "\nrun {}  started {}  baseline {}",
            run.run,
            crate::store::timestamp_id(run.started_at),
            score(run.baseline.as_ref().and_then(|b| b.mean))
        );
        for e in &run.epochs {
            let _ = writeln!(
                out,
                "  epoch {}  from {} ({})  end {}  seats {}  scoring {}",
                e.epoch,
                e.incumbent,
                score(e.incumbent_metric),
                score(e.finished_metric),
                minutes(e.hive_ms),
                minutes(e.scoring_ms)
            );
            for p in &e.candidates {
                let _ = writeln!(
                    out,
                    "    {:<16} {:<10} {}  {} seeds{}{}",
                    p.candidate,
                    p.seat.as_deref().unwrap_or("-"),
                    score(p.mean),
                    p.seeds,
                    if p.failures > 0 {
                        format!(", {} failed", p.failures)
                    } else {
                        String::new()
                    },
                    if p.promoted { "  promoted" } else { "" }
                );
            }
        }
    }
    if !history.upgrades.is_empty() {
        out.push_str("\nupgrades\n");
    }
    for u in &history.upgrades {
        let before = u.before.as_ref().map_or_else(|| "-".to_owned(), brief);
        let after = u
            .after
            .as_ref()
            .map_or_else(|| "next run".to_owned(), brief);
        let _ = writeln!(
            out,
            "  {}  {}\n    before {before}\n    after  {after}",
            u.id, u.title
        );
    }
    out
}

/// The CSV header [`csv`] writes.
pub const CSV_HEADER: &str = "run,started_at,epoch,kind,candidate,seat,mean,failures,seeds,promoted,incumbent,incumbent_metric,seats_s,scoring_s,upgrades";

fn field(text: &str) -> String {
    if text.contains([',', '"', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

fn opt<T: ToString>(value: Option<T>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

/// The history as CSV, one row per baseline or candidate. `upgrades` lists
/// the ids of the upgrades that apply from the row's run, `;`-separated.
#[must_use]
pub fn csv(history: &ScoreHistory) -> String {
    let mut out = format!("{CSV_HEADER}\n");
    for run in &history.runs {
        let applied: Vec<&str> = history
            .upgrades
            .iter()
            .filter(|u| u.run.as_deref() == Some(run.run.as_str()))
            .map(|u| u.id.as_str())
            .collect();
        let applied = field(&applied.join(";"));
        let mut row = |epoch: u32,
                       kind: &str,
                       p: &ScorePoint,
                       incumbent: &str,
                       metric: Option<f64>,
                       seats: Option<u64>,
                       scoring: Option<u64>| {
            let _ = writeln!(
                out,
                "{},{},{epoch},{kind},{},{},{},{},{},{},{},{},{},{},{applied}",
                field(&run.run),
                run.started_at,
                field(&p.candidate),
                field(p.seat.as_deref().unwrap_or("")),
                opt(p.mean),
                p.failures,
                p.seeds,
                p.promoted,
                field(incumbent),
                opt(metric),
                opt(seats.map(|ms| ms / 1000)),
                opt(scoring.map(|ms| ms / 1000)),
            );
        };
        if let Some(b) = &run.baseline {
            row(0, "baseline", b, &b.candidate, b.mean, None, None);
        }
        for e in &run.epochs {
            for p in &e.candidates {
                row(
                    e.epoch,
                    "candidate",
                    p,
                    &e.incumbent,
                    e.incumbent_metric,
                    e.hive_ms,
                    e.scoring_ms,
                );
            }
        }
    }
    out
}
