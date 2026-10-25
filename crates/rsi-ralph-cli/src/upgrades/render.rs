//! The upgrade log as Markdown, in the shape of a hand-kept `UPGRADES.md`:
//! one section per upgrade, oldest first, saying what was seen, what was
//! changed and why, what it should do, and whether it did; and one section
//! per run start, saying what the run carried and what changed unexplained.

use std::{collections::BTreeMap, fmt::Write};

use super::{Applied, Author, Contributor, Entry, Evidence, Outcome, Role, Upgrade, measured_value};

/// Render every entry of a loop's log.
#[must_use]
pub fn markdown(loop_name: &str, entries: &[Entry]) -> String {
    let mut outcomes: BTreeMap<&str, Vec<&Outcome>> = BTreeMap::new();
    let mut carried_by: BTreeMap<&str, &str> = BTreeMap::new();
    for entry in entries {
        match entry {
            Entry::Outcome(o) => outcomes.entry(o.upgrade.as_str()).or_default().push(o),
            Entry::Applied(a) => {
                for id in &a.upgrades {
                    carried_by.entry(id.as_str()).or_insert(a.run.as_str());
                }
            }
            Entry::Upgrade(_) => {}
        }
    }
    let mut sections: Vec<(u64, usize, String)> = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        let text = match entry {
            Entry::Upgrade(u) => {
                let from = u
                    .applies_from
                    .run
                    .as_deref()
                    .or_else(|| carried_by.get(u.id.as_str()).copied());
                let judged = outcomes.get(u.id.as_str()).map(Vec::as_slice).unwrap_or_default();
                upgrade(u, from, judged)
            }
            Entry::Applied(a) => applied(a),
            Entry::Outcome(_) => continue,
        };
        let at = match entry {
            Entry::Upgrade(u) => u.recorded_at,
            Entry::Applied(a) => a.recorded_at,
            Entry::Outcome(o) => o.recorded_at,
        };
        sections.push((at, i, text));
    }
    sections.sort_by_key(|(at, i, _)| (*at, *i));
    let mut out = format!("# Upgrades: {loop_name}\n");
    if sections.is_empty() {
        out.push_str("\nNo upgrades recorded.\n");
    }
    for (_, _, text) in sections {
        out.push('\n');
        out.push_str(&text);
    }
    out
}

/// `operator steven`, `assistant claude-code (claude-opus-5-5)`, `seat lead`,
/// `loop run_start_diff`.
#[must_use]
pub fn author(author: &Author) -> String {
    match author {
        Author::Operator { name } => format!("operator {name}"),
        Author::Assistant { name, model } => match model {
            Some(model) => format!("assistant {name} ({model})"),
            None => format!("assistant {name}"),
        },
        Author::Hive { seat } => format!("seat {seat}"),
        Author::Loop { mechanism } => format!("loop {mechanism}"),
    }
}

fn role(role: Role) -> &'static str {
    match role {
        Role::Authored => "authored",
        Role::Diagnosed => "diagnosed",
        Role::Proposed => "proposed",
        Role::Approved => "approved",
        Role::Applied => "applied",
        Role::Detected => "detected",
    }
}

fn by(contributors: &[Contributor]) -> String {
    contributors
        .iter()
        .map(|c| format!("{}, {}", author(&c.author), role(c.role)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn short(hash: Option<&String>) -> &str {
    hash.map_or("none", |h| h.get(..8).unwrap_or(h))
}

fn evidence(e: &Evidence) -> String {
    let mut parts = Vec::new();
    match (&e.metric, e.value) {
        (Some(m), Some(v)) => parts.push(format!("`{m}` = {v}")),
        (Some(m), None) => parts.push(format!("`{m}`")),
        _ => {}
    }
    if let Some(note) = &e.note {
        parts.push(note.clone());
    }
    let mut line = parts.join(": ");
    let mut refs = Vec::new();
    if let Some(run) = &e.run {
        refs.push(format!("run {run}"));
    }
    if let Some(claim) = &e.claim {
        refs.push(format!("claim {claim}"));
    }
    if !refs.is_empty() {
        if !line.is_empty() {
            line.push(' ');
        }
        let _ = write!(line, "({})", refs.join(", "));
    }
    format!("- {line}\n")
}

fn files(out: &mut String, files: &[super::FileChange]) {
    for f in files {
        let keys = if f.keys.is_empty() {
            String::new()
        } else {
            format!(" keys {}", f.keys.join(", "))
        };
        let hashes = if f.before.is_none() && f.after.is_none() {
            String::new()
        } else {
            format!(
                " ({} -> {})",
                short(f.before.as_ref()),
                short(f.after.as_ref())
            )
        };
        let _ = writeln!(out, "- `{}`{keys}{hashes}", f.path);
    }
}

fn upgrade(u: &Upgrade, from: Option<&str>, outcomes: &[&Outcome]) -> String {
    let mut out = format!("## {}\n\n", u.title);
    let when = match (from, u.applies_from.epoch) {
        (Some(run), Some(epoch)) => format!("from run {run}, epoch {epoch}"),
        (Some(run), None) => format!("from run {run}"),
        (None, Some(epoch)) => format!("from the next run, epoch {epoch}"),
        (None, None) => "from the next run".to_owned(),
    };
    let _ = writeln!(out, "`{}` · {when} · by {}", u.id, by(&u.contributors));
    if !u.evidence.is_empty() {
        out.push_str("\n**Saw**\n\n");
        for e in &u.evidence {
            out.push_str(&evidence(e));
        }
    }
    if let Some(diagnosis) = &u.diagnosis {
        let _ = write!(out, "\n**Diagnosis.** {diagnosis}\n");
    }
    let _ = write!(out, "\n**Change.** {}\n", u.change.summary);
    if !u.change.files.is_empty() {
        out.push('\n');
        files(&mut out, &u.change.files);
    }
    if let Some(diff) = &u.change.diff {
        let _ = write!(out, "\n```text\n{}\n```\n", diff.trim_end());
    }
    let reasons: Vec<&Contributor> = u.contributors.iter().filter(|c| c.rationale.is_some()).collect();
    if !reasons.is_empty() {
        out.push_str("\n**Why**\n\n");
        for c in reasons {
            let _ = writeln!(
                out,
                "- {}: {}",
                author(&c.author),
                c.rationale.as_deref().unwrap_or_default()
            );
        }
    }
    if !u.expected.is_empty() {
        out.push_str("\n**Expected**\n\n");
        for p in &u.expected {
            let note = p.note.as_ref().map(|n| format!(" ({n})")).unwrap_or_default();
            let _ = writeln!(out, "- `{}` {} {}{note}", p.metric, p.op.symbol(), p.value);
        }
    }
    for o in outcomes {
        outcome(&mut out, u, o);
    }
    out
}

fn outcome(out: &mut String, u: &Upgrade, o: &Outcome) {
    let verdict = match o.verdict {
        super::Verdict::Confirmed => "confirmed",
        super::Verdict::Refuted => "refuted",
        super::Verdict::Mixed => "mixed",
    };
    let _ = write!(out, "\n**Outcome in run {}: {verdict}**", o.run);
    if !o.contributors.is_empty() {
        let _ = write!(out, " (by {})", by(&o.contributors));
    }
    out.push_str("\n\n");
    for p in &u.expected {
        match measured_value(&o.measured, &p.metric) {
            Some(v) => {
                let held = if p.op.holds(v, p.value) { "held" } else { "did not hold" };
                let _ = writeln!(
                    out,
                    "- `{}` = {v}: expected {} {}, {held}",
                    p.metric,
                    p.op.symbol(),
                    p.value
                );
            }
            None => {
                let _ = writeln!(out, "- `{}`: not measured", p.metric);
            }
        }
    }
    for e in o
        .measured
        .iter()
        .filter(|e| !e.metric.as_ref().is_some_and(|m| u.expected.iter().any(|p| &p.metric == m)))
    {
        out.push_str(&evidence(e));
    }
    for c in o.contributors.iter().filter(|c| c.rationale.is_some()) {
        let _ = writeln!(
            out,
            "- {}: {}",
            author(&c.author),
            c.rationale.as_deref().unwrap_or_default()
        );
    }
}

fn applied(a: &Applied) -> String {
    let mut out = format!("## Run {} started\n\n", a.run);
    match &a.previous_run {
        None => out.push_str("The loop's first stored run.\n"),
        Some(previous) if a.files.is_empty() => {
            let _ = writeln!(out, "Hive and loop config unchanged since run {previous}.");
        }
        Some(previous) => {
            let _ = writeln!(
                out,
                "{} changed since run {previous}:\n",
                plural(a.files.len(), "file", "files")
            );
            files(&mut out, &a.files);
        }
    }
    if !a.upgrades.is_empty() {
        let ids: Vec<String> = a.upgrades.iter().map(|id| format!("`{id}`")).collect();
        let _ = write!(out, "\nCarries {}.\n", ids.join(", "));
    }
    if !a.unexplained.is_empty() {
        let paths: Vec<String> = a.unexplained.iter().map(|p| format!("`{p}`")).collect();
        let _ = write!(out, "\nNo upgrade record explains {}.\n", paths.join(", "));
    }
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
