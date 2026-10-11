//! The supervisor prompt: the goal, metric history, approaches already tried,
//! earlier proposals this epoch, recent rejections, the incumbent's sources
//! and the slot's directive, rendered from the loop's own ledger.
//!
//! The loop renders it because the ledger is the loop's; the supervisor in
//! `tinyrsi-evolve` only sends it.
use super::{
    LedgerCandidate, LoopPolicy, ScriptLedger, ScriptSubject, SeedRun, approaches, execute::Sources,
};
use std::fmt::Write as _;

/// Which proposal of which epoch a prompt is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Slot {
    /// Epoch number.
    pub epoch: u64,
    /// Slot index within the epoch, from 0.
    pub index: u32,
}

/// Builds the supervisor prompt for one slot from the subject, ledger,
/// incumbent and policy.
pub(super) fn prompt(
    subject: &ScriptSubject,
    ledger: &ScriptLedger,
    sources: &Sources,
    policy: &LoopPolicy,
    slot: Slot,
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
    text.push('\n');
    text.push_str(&approaches::render_tried(
        ledger,
        metric.direction,
        policy.approach_context,
    ));
    text.push_str(&approaches::render_this_epoch(ledger, slot.epoch));
    text.push_str(&rejected(ledger, policy.rejected_context));
    let _ = writeln!(text, "\n# Incumbent editable sources");
    for (path, source) in sources {
        let _ = writeln!(
            text,
            "\n## {}\n```\n{}\n```",
            path.display(),
            source.trim_end()
        );
    }
    if let Some(directive) = policy.slot(slot.index) {
        let _ = writeln!(
            text,
            "\n# This proposal (slot {} of {})\n{}",
            slot.index.saturating_add(1),
            policy.proposals_per_epoch,
            directive.instruction.trim()
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

/// The "Recently rejected proposals" section: the last `limit` proposals
/// that were not accepted, newest first.
fn rejected(ledger: &ScriptLedger, limit: usize) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "# Recently rejected proposals");
    let rejected: Vec<&LedgerCandidate> = ledger
        .candidates
        .iter()
        .rev()
        .filter(|c| !c.promoted && c.proposal_id.is_some())
        .filter(|c| c.decision != super::CandidateDecision::Accept)
        .take(limit)
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
            "- {:?} ({}): decision {:?}, mean {:?}, errors {:?}, note {:?}\n  rationale: {}\n  hypothesis: {}\n  stderr tail: {}\n  diagnostics: {}",
            c.proposal_id.as_deref().unwrap_or_default(),
            c.approach.as_deref().unwrap_or("unlabelled"),
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
    text
}
