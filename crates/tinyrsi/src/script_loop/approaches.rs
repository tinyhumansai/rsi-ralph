//! The hypothesis ledger: what the supervisor has already tried, grouped by
//! approach label, and what earlier slots of the current epoch proposed.
//!
//! Both are rendered from the loop's own ledger into the supervisor prompt so
//! a ruled-out idea stays visible after it leaves the recent-rejections
//! window, and two wordings of one idea count as one approach. Behavior is
//! specified in `docs/specs/hypothesis-ledger.md`.
use super::{CandidateDecision, LedgerCandidate, ScriptLedger};
use crate::MetricDirection;
use std::fmt::Write as _;
use tinyrsi_score::MetricScorer;

/// Longest verdict, in characters, shown for an approach.
pub(super) const VERDICT_CHARS: usize = 160;

/// One approach as the prompt shows it: every attempt with the same
/// normalized label, summarized.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Approach {
    /// The most recent attempt's label, as the supervisor wrote it.
    pub label: String,
    /// Proposals made with this approach.
    pub attempts: u32,
    /// Best mean metric over the measured attempts.
    pub best: Option<f64>,
    /// The most recent attempt's decision.
    pub decision: CandidateDecision,
    /// The most recent attempt's epoch.
    pub epoch: u64,
    /// One line on how the most recent attempt went.
    pub verdict: String,
    /// What the most recent attempt said it changed, when it said so.
    pub differs: Option<String>,
    /// Ledger index of the most recent attempt, for recency order.
    last: usize,
}

/// Lowercase, with every run of characters that are not letters or digits
/// turned into one space, trimmed: `"Tabu-Search!"` is `"tabu search"`.
pub(super) fn normalize(label: &str) -> String {
    let spaced: String = label
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .flat_map(char::to_lowercase)
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The ledger's labelled proposals grouped by normalized approach, most
/// recently attempted first, at most `limit` of them.
pub(super) fn summarize(
    ledger: &ScriptLedger,
    direction: MetricDirection,
    limit: usize,
) -> Vec<Approach> {
    let metric = MetricScorer::new(direction);
    let mut groups: Vec<(String, Approach)> = vec![];
    for (index, row) in ledger.candidates.iter().enumerate() {
        let Some(label) = row.approach.as_deref().filter(|_| row.proposal_id.is_some()) else {
            continue;
        };
        let key = normalize(label);
        if key.is_empty() {
            continue;
        }
        let latest = Approach {
            label: label.to_owned(),
            attempts: 1,
            best: row.mean,
            decision: row.decision,
            epoch: row.epoch,
            verdict: verdict(row),
            differs: row.differs_from_previous.clone(),
            last: index,
        };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, group)) => {
                let best = match (group.best, row.mean) {
                    (Some(best), Some(mean)) if !metric.better(mean, best) => Some(best),
                    (best, mean) => mean.or(best),
                };
                *group = Approach {
                    attempts: group.attempts.saturating_add(1),
                    best,
                    ..latest
                };
            }
            None => groups.push((key, latest)),
        }
    }
    let mut approaches: Vec<Approach> = groups.into_iter().map(|(_, a)| a).collect();
    approaches.sort_by(|a, b| b.last.cmp(&a.last));
    approaches.truncate(limit);
    approaches
}

/// How the attempt went, on one line: its first seed error, else its note,
/// else the last `DIAG` line of its first seed with diagnostics.
fn verdict(row: &LedgerCandidate) -> String {
    let text = row
        .runs
        .iter()
        .find_map(|r| r.error.as_deref())
        .map(|e| format!("error: {e}"))
        .or_else(|| row.note.clone())
        .or_else(|| {
            row.runs
                .iter()
                .find_map(|r| r.diagnostics.last())
                .cloned()
        })
        .unwrap_or_else(|| "no diagnostics".to_owned());
    one_line(&text, VERDICT_CHARS)
}

/// `text` with whitespace runs collapsed, cut to `chars` characters (the last
/// one an ellipsis).
pub(super) fn one_line(text: &str, chars: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= chars {
        return line;
    }
    let mut cut: String = line.chars().take(chars.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// The ledger's name for a decision, as `ledger.json` spells it.
pub(super) fn decision_name(decision: CandidateDecision) -> &'static str {
    match decision {
        CandidateDecision::Baseline => "baseline",
        CandidateDecision::Accept => "accept",
        CandidateDecision::Reject => "reject",
        CandidateDecision::Inconclusive => "inconclusive",
        CandidateDecision::Invalid => "invalid",
        CandidateDecision::Duplicate => "duplicate",
        CandidateDecision::Error => "error",
    }
}

/// A mean metric, or `none` when no seed produced one.
fn mean(value: Option<f64>) -> String {
    value.map_or_else(|| "none".to_owned(), |v| v.to_string())
}

/// The "Approaches already tried" section and its resubmission rule, or an
/// empty string when `limit` is zero.
pub(super) fn render_tried(
    ledger: &ScriptLedger,
    direction: MetricDirection,
    limit: usize,
) -> String {
    if limit == 0 {
        return String::new();
    }
    let mut text = String::from("# Approaches already tried (most recent first)\n");
    let approaches = summarize(ledger, direction, limit);
    if approaches.is_empty() {
        text.push_str("None labelled yet.\n");
    }
    for a in &approaches {
        let _ = writeln!(
            text,
            "- \"{}\": {} attempt{}, best mean {}, last decision {} (epoch {}); verdict: {}",
            a.label,
            a.attempts,
            if a.attempts == 1 { "" } else { "s" },
            mean(a.best),
            decision_name(a.decision),
            a.epoch,
            a.verdict
        );
        if let Some(differs) = &a.differs {
            let _ = writeln!(
                text,
                "  last said it differed by: {}",
                one_line(differs, VERDICT_CHARS)
            );
        }
    }
    text.push_str(
        "Do not resubmit an approach whose last decision was not accept unless your \
         proposal's `differs_from_previous` states concretely what is different this time.\n\n",
    );
    text
}

/// The "Earlier proposals this epoch" section, or an empty string when this
/// is the epoch's first proposal.
pub(super) fn render_this_epoch(ledger: &ScriptLedger, epoch: u64) -> String {
    let earlier: Vec<&LedgerCandidate> = ledger
        .candidates
        .iter()
        .filter(|c| c.epoch == epoch && c.proposal_id.is_some())
        .collect();
    if earlier.is_empty() {
        return String::new();
    }
    let mut text = String::from("# Earlier proposals this epoch\n");
    for c in earlier {
        let _ = writeln!(
            text,
            "- {}: approach {}, decision {}, mean {}",
            c.proposal_id.as_deref().unwrap_or_default(),
            c.approach
                .as_deref()
                .map_or_else(|| "unlabelled".to_owned(), |a| format!("\"{a}\"")),
            decision_name(c.decision),
            mean(c.mean)
        );
    }
    text.push('\n');
    text
}
