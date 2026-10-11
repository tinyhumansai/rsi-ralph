//! The benchmark harness: run benchmark × model × repeat and compare.
//!
//! The pure measurement lives here: [`record`] turns one run's
//! `ledger.json` into a [`RunRecord`], [`repetition`] counts proposals that
//! repeat an idea already ruled out in the run, [`aggregate`] folds records
//! into one [`SummaryRow`] per benchmark × model, and [`render_markdown`]
//! renders the comparison table. [`options`] parses the command line and
//! [`runner`] executes the matrix. Every definition is in
//! `docs/specs/loop-benchmarks.md`.
//!
//! # Example
//!
//! ```
//! use tinyrsi_examples::harness::{model_label, normalize_hypothesis};
//! assert_eq!(model_label("anthropic/claude-sonnet-4.5"), "anthropic_claude-sonnet-4.5");
//! assert_eq!(normalize_hypothesis("Kempe chains, again!"), "kempe chains again");
//! ```
pub mod options;
pub mod runner;
mod types;

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::PathBuf;
use tinyrsi::{CandidateDecision, LedgerCandidate, ScriptLedger, StopReason};
pub use types::{RunCell, RunRecord, Summary, SummaryRow};

/// `model` with every character outside `[A-Za-z0-9._-]` replaced by `_`,
/// for use as a directory name.
#[must_use]
pub fn model_label(model: &str) -> String {
    model
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// A hypothesis lowercased, with every run of non-alphanumeric characters
/// replaced by one space, and trimmed.
#[must_use]
pub fn normalize_hypothesis(hypothesis: &str) -> String {
    let mut out = String::with_capacity(hypothesis.len());
    for c in hypothesis.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.truncate(out.trim_end().len());
    out
}

/// `(repeats, proposals)` over a run's ledger rows.
///
/// Proposals are the rows after the baseline. A proposal is a repeat when an
/// earlier ruled-out proposal (not promoted, not itself a duplicate) has the
/// same content id or the same non-empty normalized hypothesis.
#[must_use]
pub fn repetition(candidates: &[LedgerCandidate]) -> (u32, u32) {
    let mut ids = HashSet::new();
    let mut hypotheses = HashSet::new();
    let (mut repeats, mut proposals) = (0, 0);
    for candidate in candidates.iter().filter(|c| c.epoch > 0) {
        proposals += 1;
        let hypothesis = normalize_hypothesis(&candidate.hypothesis);
        let same_content = candidate.id.as_ref().is_some_and(|id| ids.contains(id));
        let same_idea = !hypothesis.is_empty() && hypotheses.contains(&hypothesis);
        if same_content || same_idea {
            repeats += 1;
        }
        let ruled_out =
            !candidate.promoted && candidate.decision != CandidateDecision::Duplicate;
        if ruled_out {
            ids.extend(candidate.id.clone());
            if !hypothesis.is_empty() {
                hypotheses.insert(hypothesis);
            }
        }
    }
    (repeats, proposals)
}

/// The record of one run: its matrix cell, what its ledger shows (if it
/// wrote one), its wall time, directory, and error message if it failed.
#[must_use]
pub fn record(
    cell: RunCell,
    ledger: Option<&ScriptLedger>,
    wall_ms: u64,
    run_dir: PathBuf,
    error: Option<String>,
) -> RunRecord {
    let empty = [];
    let candidates = ledger.map_or(&empty[..], |l| l.candidates.as_slice());
    let proposals = || candidates.iter().filter(|c| c.epoch > 0);
    let count = |pred: &dyn Fn(&LedgerCandidate) -> bool| {
        u32::try_from(proposals().filter(|c| pred(c)).count()).unwrap_or(u32::MAX)
    };
    let stop = match (&error, ledger.and_then(|l| l.stop)) {
        (None, Some(stop)) => stop,
        _ => StopReason::Error,
    };
    let (repeats, proposal_count) = repetition(candidates);
    let incumbents = ledger.map_or(&[][..], |l| l.incumbents.as_slice());
    RunRecord {
        benchmark: cell.benchmark,
        model: cell.model,
        repeat: cell.repeat,
        stop,
        goal_reached: stop == StopReason::GoalReached,
        epochs: candidates.iter().map(|c| c.epoch).max().unwrap_or(0),
        metric: ledger.map_or_else(String::new, |l| l.metric.name.clone()),
        baseline_metric: incumbents.first().map(|i| i.metric),
        final_metric: incumbents.last().map(|i| i.metric),
        promotions: u32::try_from(incumbents.len().saturating_sub(1)).unwrap_or(u32::MAX),
        proposals: proposal_count,
        rejected: count(&|c| c.decision != CandidateDecision::Accept),
        duplicates: count(&|c| c.decision == CandidateDecision::Duplicate),
        invalid: count(&|c| c.decision == CandidateDecision::Invalid),
        repeats,
        repetition_share: (proposal_count > 0)
            .then(|| f64::from(repeats) / f64::from(proposal_count)),
        tokens: ledger.map_or(0, |l| l.tokens),
        wall_ms,
        run_dir,
        error,
    }
}

/// One row per benchmark × model, in the order cells first appear.
#[must_use]
pub fn aggregate(records: &[RunRecord]) -> Vec<SummaryRow> {
    let mut cells: Vec<(&str, &str)> = vec![];
    for r in records {
        if !cells.contains(&(r.benchmark.as_str(), r.model.as_str())) {
            cells.push((&r.benchmark, &r.model));
        }
    }
    cells
        .into_iter()
        .map(|(benchmark, model)| {
            let runs: Vec<&RunRecord> = records
                .iter()
                .filter(|r| r.benchmark == benchmark && r.model == model)
                .collect();
            let goal_runs: Vec<&&RunRecord> = runs.iter().filter(|r| r.goal_reached).collect();
            let n = u32::try_from(runs.len()).unwrap_or(u32::MAX);
            let goals = u32::try_from(goal_runs.len()).unwrap_or(u32::MAX);
            SummaryRow {
                benchmark: benchmark.into(),
                model: model.into(),
                metric: runs
                    .iter()
                    .find(|r| !r.metric.is_empty())
                    .map_or_else(String::new, |r| r.metric.clone()),
                runs: n,
                goals,
                goal_rate: if n == 0 {
                    0.0
                } else {
                    f64::from(goals) / f64::from(n)
                },
                median_epochs_to_goal: median(goal_runs.iter().map(|r| lossy(r.epochs)).collect()),
                median_tokens: median(runs.iter().map(|r| lossy(r.tokens)).collect()),
                median_final_metric: median(runs.iter().filter_map(|r| r.final_metric).collect()),
                mean_repetition_share: mean(runs.iter().filter_map(|r| r.repetition_share)),
                median_wall_ms: median(runs.iter().map(|r| lossy(r.wall_ms)).collect()),
            }
        })
        .collect()
}

/// The comparison table as Markdown; absent values render as `–`.
#[must_use]
pub fn render_markdown(rows: &[SummaryRow]) -> String {
    let mut out = String::from(
        "| Benchmark | Model | Goal | Epochs to goal | Tokens | Final metric | Repetition | Wall (s) |\n\
         | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |\n",
    );
    let dash = || "–".to_owned();
    for row in rows {
        let metric = row
            .median_final_metric
            .map_or_else(dash, |v| format!("{} {}", number(v), row.metric));
        let _ = writeln!(
            out,
            "| {} | {} | {}/{} | {} | {} | {} | {} | {} |",
            row.benchmark,
            row.model,
            row.goals,
            row.runs,
            row.median_epochs_to_goal.map_or_else(dash, number),
            row.median_tokens.map_or_else(dash, number),
            metric,
            row.mean_repetition_share
                .map_or_else(dash, |s| format!("{:.0}%", s * 100.0)),
            row.median_wall_ms
                .map_or_else(dash, |ms| format!("{:.1}", ms / 1000.0)),
        );
    }
    out
}

/// A number with at most four decimals and no trailing zeros.
fn number(value: f64) -> String {
    let text = format!("{value:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// `u64` to `f64` for medians; exact below 2^53, which counts here never
/// reach.
#[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^53")]
fn lossy(value: u64) -> f64 {
    value as f64
}

/// The median; the mean of the middle two for an even count.
fn median(mut values: Vec<f64>) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => values.get(n / 2).copied(),
        _ => Some((values.get(n / 2 - 1)? + values.get(n / 2)?) / 2.0),
    }
}

/// The arithmetic mean, absent for no values.
fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = values.fold((0.0, 0.0), |(s, n), v| (s + v, n + 1.0));
    (n > 0.0).then(|| sum / n)
}

#[cfg(test)]
mod test;
