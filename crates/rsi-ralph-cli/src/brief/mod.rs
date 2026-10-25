//! Rendering `brief.md` for one epoch.
//!
//! The template is the user's Markdown. Five placeholders are filled in:
//! `{{incumbent}}` (current scores), `{{history}}` (earlier decisions in this
//! run), `{{failures}}` (a sample of what the incumbent gets wrong),
//! `{{ideas}}` (creative-retreat ideas) and `{{notes}}` (what every seat of
//! the last epoch wrote, when the loop carries notes; see [`crate::carry`]).
//! A placeholder the template leaves out is appended as its own section when
//! it has content, so a plain brief still gets the evidence.

use std::{collections::BTreeMap, fmt::Write as _};

use crate::{score::Scorecard, step::Failure};

/// Most failures shown in one brief.
const MAX_FAILURES: usize = 10;
/// Most failures of one type shown in one brief.
const PER_TYPE: usize = 2;
/// Longest failure text shown, in characters.
const FAILURE_CHARS: usize = 1500;

/// One earlier decision, for `{{history}}`.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    /// Epoch it was decided in.
    pub epoch: u32,
    /// Candidate id.
    pub candidate: String,
    /// Outcome in a word or two: `promoted`, `rejected`, `empty`, ...
    pub outcome: String,
    /// The reason recorded with it.
    pub reason: String,
}

/// What a brief is rendered from.
#[derive(Debug, Clone, Copy)]
pub struct BriefInput<'a> {
    /// The incumbent's scorecard.
    pub incumbent: &'a Scorecard,
    /// The deciding scorer.
    pub decide_on: &'a str,
    /// Earlier decisions in this run.
    pub history: &'a [HistoryEntry],
    /// Retreat ideas for this epoch.
    pub ideas: &'a [String],
    /// The last epoch's notes, already rendered by [`crate::carry::render`];
    /// empty when there are none.
    pub notes: &'a str,
}

/// Fill the template.
#[must_use]
pub fn render(template: &str, input: &BriefInput<'_>) -> String {
    let sections = [
        ("incumbent", "Where we are", incumbent(input)),
        ("history", "What has been tried", history(input.history)),
        ("failures", "What still fails", failures(input)),
        ("ideas", "Ideas from the retreat", ideas(input.ideas)),
        ("notes", "Notes from the last epoch", input.notes.to_owned()),
    ];
    let mut out = template.to_owned();
    for (key, title, body) in sections {
        let placeholder = format!("{{{{{key}}}}}");
        if out.contains(&placeholder) {
            let body = if body.is_empty() {
                "None yet.".to_owned()
            } else {
                body
            };
            out = out.replace(&placeholder, body.trim_end());
        } else if !body.is_empty() {
            let _ = write!(out, "\n\n## {title}\n\n{}", body.trim_end());
        }
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn incumbent(input: &BriefInput<'_>) -> String {
    let card = input.incumbent;
    let mut out = String::new();
    if let Some(headline) = card.headline {
        let _ = writeln!(out, "- `{}` (deciding): {headline:.4}", input.decide_on);
    }
    for (id, runs) in &card.runs {
        if id == input.decide_on {
            continue;
        }
        let scores: Vec<String> = runs
            .iter()
            .filter_map(|r| r.result.as_ref().map(|x| format!("{:.4}", x.score)))
            .collect();
        if !scores.is_empty() {
            let _ = writeln!(out, "- `{id}`: {}", scores.join(", "));
        }
    }
    let metrics: BTreeMap<&String, f64> = card
        .runs
        .get(input.decide_on)
        .and_then(|r| r.first())
        .and_then(|r| r.result.as_ref())
        .map(|r| r.metrics.iter().map(|(k, v)| (k, *v)).collect())
        .unwrap_or_default();
    for (name, value) in metrics {
        let _ = writeln!(out, "- {name}: {value}");
    }
    out
}

fn history(entries: &[HistoryEntry]) -> String {
    entries.iter().fold(String::new(), |mut out, e| {
        let _ = writeln!(
            out,
            "- epoch {} `{}`: {} ({})",
            e.epoch, e.candidate, e.outcome, e.reason
        );
        out
    })
}

fn failures(input: &BriefInput<'_>) -> String {
    let all: Vec<&Failure> = input
        .incumbent
        .runs
        .get(input.decide_on)
        .into_iter()
        .flatten()
        .filter_map(|r| r.result.as_ref())
        .flat_map(|r| &r.failures)
        .collect();
    let mut per_type: BTreeMap<&str, usize> = BTreeMap::new();
    let mut out = String::new();
    let mut shown = 0;
    for failure in all {
        let kind = failure.kind.as_deref().unwrap_or("");
        let count = per_type.entry(kind).or_default();
        if *count >= PER_TYPE || shown >= MAX_FAILURES {
            continue;
        }
        *count += 1;
        shown += 1;
        let text = clip(&failure.text, FAILURE_CHARS);
        let label = if kind.is_empty() {
            failure.id.clone()
        } else {
            format!("{} ({kind})", failure.id)
        };
        let _ = write!(out, "### {label}\n\n{}\n\n", text.trim_end());
    }
    out
}

/// The first `limit` characters of `text`, marked when cut, with a code fence
/// the cut left open closed again so the rest of the brief still renders.
pub(crate) fn clip(text: &str, limit: usize) -> String {
    let mut out: String = text.chars().take(limit).collect();
    if out.len() < text.len() {
        out.push('…');
        let fences = out
            .lines()
            .filter(|line| line.trim_start().starts_with("```"))
            .count();
        if fences % 2 == 1 {
            out.push_str("\n```");
        }
    }
    out
}

fn ideas(ideas: &[String]) -> String {
    ideas.iter().fold(String::new(), |mut out, i| {
        let _ = writeln!(out, "- {i}");
        out
    })
}

#[cfg(test)]
mod test;
