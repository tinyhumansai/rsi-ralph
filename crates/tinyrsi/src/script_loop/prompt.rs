//! The supervisor prompt: the goal, metric history, recent rejections and the
//! incumbent's sources, rendered from the loop's own ledger.
//!
//! The loop renders it because the ledger is the loop's; the supervisor in
//! `tinyrsi-evolve` only sends it.
use super::{LedgerCandidate, ScriptLedger, ScriptSubject, SeedRun, execute::Sources};
use std::fmt::Write as _;

/// Builds the supervisor prompt from the subject, ledger and incumbent.
pub(super) fn prompt(
    subject: &ScriptSubject,
    ledger: &ScriptLedger,
    sources: &Sources,
    rejected_context: usize,
) -> String {
    let metric = &subject.metric;
    let mut text = String::new();
    let _ = writeln!(text, "# Goal\n{}\n", subject.brief);
    let _ = writeln!(
        text,
        "Metric `{}` ({:?}); target: {}. The script prints `METRIC {}=<value>` on stdout.",
        metric.name,
        metric.direction,
        metric
            .target
            .map_or_else(|| "none".to_owned(), |t| t.to_string()),
        metric.name
    );
    let _ = writeln!(
        text,
        "Command: {} {} (run once per seed {:?} with TINYRSI_SEED set; cleared environment).\n\
         Lines printed to stdout as `DIAG <text>` are collected per seed and shown below. \
         Print them from your code to report what it did (moves tried, conflicts left, \
         residuals): it is the only view you get of a run besides its metric.\n",
        subject.command.program.display(),
        subject.command.args.join(" "),
        subject.seeds
    );
    let _ = writeln!(text, "# Metric history (incumbent per epoch)");
    for entry in &ledger.incumbents {
        let _ = writeln!(
            text,
            "- epoch {}: {} = {}",
            entry.epoch, entry.id, entry.metric
        );
    }
    let _ = writeln!(text, "\n# Incumbent diagnostics (DIAG lines per seed)");
    let incumbent = ledger.incumbents.last().and_then(|i| {
        ledger
            .candidates
            .iter()
            .find(|c| c.id.as_deref() == Some(&i.id))
    });
    match incumbent {
        Some(c) if c.runs.iter().any(|r| !r.diagnostics.is_empty()) => {
            for run in &c.runs {
                let _ = writeln!(text, "- seed {}: {}", run.seed, diagnostics(run));
            }
        }
        _ => {
            let _ = writeln!(text, "No diagnostics recorded for the incumbent.");
        }
    }
    let _ = writeln!(text, "\n# Recently rejected proposals");
    let rejected: Vec<&LedgerCandidate> = ledger
        .candidates
        .iter()
        .rev()
        .filter(|c| !c.promoted && c.proposal_id.is_some())
        .filter(|c| c.decision != super::CandidateDecision::Accept)
        .take(rejected_context)
        .collect();
    if rejected.is_empty() {
        let _ = writeln!(text, "None yet.");
    }
    for c in rejected {
        let shown = c.runs.iter().find(|r| r.error.is_some()).or(c.runs.first());
        let stderr = shown.map_or("", |r| r.stderr_tail.as_str());
        let errors: Vec<&str> = c.runs.iter().filter_map(|r| r.error.as_deref()).collect();
        let _ = writeln!(
            text,
            "- {:?}: decision {:?}, mean {:?}, errors {:?}, note {:?}\n  rationale: {}\n  hypothesis: {}\n  stderr tail: {}\n  diagnostics: {}",
            c.proposal_id.as_deref().unwrap_or_default(),
            c.decision,
            c.mean,
            errors,
            c.note.as_deref().unwrap_or_default(),
            c.rationale,
            c.hypothesis,
            stderr.trim(),
            shown.map_or_else(|| "(none)".to_owned(), diagnostics)
        );
    }
    let _ = writeln!(text, "\n# Incumbent editable sources");
    for (path, source) in sources {
        let _ = writeln!(
            text,
            "\n## {}\n```\n{}\n```",
            path.display(),
            source.trim_end()
        );
    }
    text
}

/// One seed's diagnostics on one line, or `(none)`.
fn diagnostics(run: &SeedRun) -> String {
    if run.diagnostics.is_empty() {
        "(none)".to_owned()
    } else {
        run.diagnostics.join(" | ")
    }
}
