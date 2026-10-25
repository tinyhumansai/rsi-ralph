//! `rsi-ralph upgrades`: print a loop's upgrade log, and record upgrades and
//! their outcomes in the loop folder's `upgrades.jsonl`.

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use clap::{Args, Subcommand};

use super::say;
use crate::{
    Error, Result, manifest,
    store::Store,
    upgrades::{
        self, AppliesFrom, Change, Contributor, Entry, Evidence, LOG, Outcome, Upgrade, drift,
        notation,
    },
};

/// The `upgrades` subcommands.
#[derive(Debug, Subcommand)]
pub enum UpgradesCommand {
    /// Print the log as Markdown: the loop folder's records and every stored
    /// run's.
    Show {
        /// The loop folder, or a loop name to read from the run store alone.
        r#loop: PathBuf,
        /// Print the entries as JSON lines instead.
        #[arg(long)]
        json: bool,
    },
    /// Record an upgrade in the loop folder's `upgrades.jsonl`.
    Add(Box<AddArgs>),
    /// Record what a run measured against an upgrade's predictions.
    Outcome(Box<OutcomeArgs>),
}

/// `rsi-ralph upgrades add`.
#[derive(Debug, Args)]
pub struct AddArgs {
    /// The loop folder.
    pub r#loop: PathBuf,
    /// One-line name of the change.
    #[arg(long, required_unless_present = "from")]
    pub title: Option<String>,
    /// Who took part: `operator:NAME`, `assistant:NAME[@MODEL]`, `hive:SEAT`
    /// or `loop:MECHANISM`, each with an optional `=ROLE` (`authored`,
    /// `diagnosed`, `proposed`, `approved`, `applied`). Repeatable.
    #[arg(
        long = "author",
        value_name = "KIND:NAME[@MODEL][=ROLE]",
        required_unless_present = "from"
    )]
    pub authors: Vec<String>,
    /// The reason in that contributor's own words; the n-th `--rationale`
    /// belongs to the n-th `--author`. Repeatable.
    #[arg(long = "rationale", value_name = "TEXT")]
    pub rationales: Vec<String>,
    /// An observation the change rests on. Repeatable.
    #[arg(long = "saw", value_name = "TEXT")]
    pub saw: Vec<String>,
    /// A measured number it rests on. Repeatable.
    #[arg(long = "metric", value_name = "METRIC=VALUE[@RUN]")]
    pub metrics: Vec<String>,
    /// A claim id it rests on. Repeatable.
    #[arg(long = "claim", value_name = "ID")]
    pub claims: Vec<String>,
    /// The root cause.
    #[arg(long)]
    pub diagnosis: Option<String>,
    /// What changed, in words.
    #[arg(long, required_unless_present = "from")]
    pub change: Option<String>,
    /// A file it touched, relative to the loop folder, with the keys inside
    /// it. Hashed before (the latest run's copy) and after (the folder's).
    /// Repeatable.
    #[arg(long = "file", value_name = "PATH[#KEY,KEY]")]
    pub files: Vec<String>,
    /// A measurable prediction. Repeatable.
    #[arg(long = "expect", value_name = "METRIC OP VALUE")]
    pub expect: Vec<String>,
    /// The run it applies from; defaults to the next run.
    #[arg(long)]
    pub run: Option<String>,
    /// The epoch it applies from within that run.
    #[arg(long)]
    pub epoch: Option<u32>,
    /// The record's id; defaults to `u-<UTC time>`.
    #[arg(long)]
    pub id: Option<String>,
    /// Read the whole record from a JSON file instead of the flags above;
    /// `id` and `recorded_at` may be left out.
    #[arg(long, value_name = "FILE", conflicts_with_all = ["title", "authors", "change"])]
    pub from: Option<PathBuf>,
}

/// `rsi-ralph upgrades outcome`.
#[derive(Debug, Args)]
pub struct OutcomeArgs {
    /// The loop folder.
    pub r#loop: PathBuf,
    /// The upgrade's id.
    pub upgrade: String,
    /// The run that measured it.
    #[arg(long)]
    pub run: String,
    /// A measured number. Repeatable.
    #[arg(long = "metric", value_name = "METRIC=VALUE[@RUN]")]
    pub metrics: Vec<String>,
    /// An observation. Repeatable.
    #[arg(long = "saw", value_name = "TEXT")]
    pub saw: Vec<String>,
    /// `confirmed`, `refuted` or `mixed`; worked out from the predictions
    /// and `--metric` when left out.
    #[arg(long)]
    pub verdict: Option<String>,
    /// Who judged it, as for `add`. Repeatable.
    #[arg(long = "author", value_name = "KIND:NAME[@MODEL][=ROLE]")]
    pub authors: Vec<String>,
    /// That judge's reasoning, paired with `--author` by position.
    /// Repeatable.
    #[arg(long = "rationale", value_name = "TEXT")]
    pub rationales: Vec<String>,
}

/// Run an `upgrades` subcommand.
pub(super) fn execute(
    command: UpgradesCommand,
    store: &Store,
    now: u64,
    out: &mut dyn Write,
) -> Result<()> {
    match command {
        UpgradesCommand::Show { r#loop, json } => show(store, &r#loop, json, out),
        UpgradesCommand::Add(args) => add(store, &args, now, out),
        UpgradesCommand::Outcome(args) => outcome(store, &args, now, out),
    }
}

/// The loop's name and its folder log, from a folder or, for a bare name,
/// from the latest stored run's frozen copy.
fn loop_log(store: &Store, target: &Path) -> Result<(String, Vec<Entry>)> {
    if target.join(manifest::MANIFEST).is_file() {
        return Ok((loop_name(target)?, upgrades::read(&target.join(LOG))?));
    }
    let name = target.to_string_lossy().into_owned();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(Error::MissingFile {
            path: target.join(manifest::MANIFEST).display().to_string(),
        });
    }
    let log = match drift::previous_run(store, &name, "")? {
        Some((_, files)) => match files.get(LOG) {
            Some(text) => upgrades::parse(text, Path::new(LOG))?,
            None => Vec::new(),
        },
        None => return Err(Error::NoRun(name)),
    };
    Ok((name, log))
}

/// `name` from a loop folder's `rsi.json`, read without validating the rest,
/// so a log stays readable while the loop is being edited.
fn loop_name(dir: &Path) -> Result<String> {
    let path = dir.join(manifest::MANIFEST);
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io("read", &path, e))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| Error::Json {
        message: e.to_string(),
    })?;
    value["name"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::field("name", "missing from rsi.json"))
}

fn show(store: &Store, target: &Path, json: bool, out: &mut dyn Write) -> Result<()> {
    let (name, log) = loop_log(store, target)?;
    let entries = upgrades::collect(store, &name, log)?;
    if json {
        for entry in &entries {
            let line = serde_json::to_string(entry).map_err(|e| Error::Json {
                message: e.to_string(),
            })?;
            say(out, &line)?;
        }
        return Ok(());
    }
    write!(out, "{}", upgrades::render::markdown(&name, &entries))
        .map_err(|e| Error::io("write", "stdout", e))
}

/// Contributors from `--author`, each given the `--rationale` at its
/// position.
fn contributors(authors: &[String], rationales: &[String]) -> Result<Vec<Contributor>> {
    if rationales.len() > authors.len() {
        return Err(Error::field(
            "--rationale",
            "each --rationale needs an --author before it",
        ));
    }
    authors
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let mut c = notation::contributor(text)?;
            c.rationale = rationales.get(i).cloned();
            Ok(c)
        })
        .collect()
}

fn evidence(saw: &[String], metrics: &[String], claims: &[String]) -> Result<Vec<Evidence>> {
    let mut out: Vec<Evidence> = saw
        .iter()
        .map(|note| Evidence {
            note: Some(note.clone()),
            ..Evidence::default()
        })
        .collect();
    for m in metrics {
        out.push(notation::metric(m)?);
    }
    out.extend(claims.iter().map(|claim| Evidence {
        claim: Some(claim.clone()),
        ..Evidence::default()
    }));
    Ok(out)
}

fn add(store: &Store, args: &AddArgs, now: u64, out: &mut dyn Write) -> Result<()> {
    let dir = &args.r#loop;
    let name = loop_name(dir)?;
    let log_path = dir.join(LOG);
    let entries = upgrades::collect(store, &name, upgrades::read(&log_path)?)?;
    let mut upgrade = match &args.from {
        Some(path) => from_file(path, &entries, now)?,
        None => from_flags(args, &entries, now)?,
    };
    if upgrade.contributors.is_empty() {
        return Err(Error::field("--author", "an upgrade needs at least one"));
    }
    if upgrades::upgrades(&entries).any(|u| u.id == upgrade.id) {
        return Err(Error::UpgradeExists(upgrade.id));
    }
    let previous = drift::previous_run(store, &name, "")?.map(|(_, files)| files);
    for file in &mut upgrade.change.files {
        if file.path.ends_with('/') {
            continue;
        }
        if file.after.is_none() {
            file.after = std::fs::read_to_string(dir.join(&file.path))
                .ok()
                .map(|t| upgrades::digest(&t));
        }
        if file.before.is_none() {
            file.before = previous
                .as_ref()
                .and_then(|files| files.get(&file.path))
                .map(|t| upgrades::digest(t));
        }
    }
    let id = upgrade.id.clone();
    upgrades::append(&log_path, &Entry::Upgrade(upgrade))?;
    say(out, &format!("recorded {id} in {}", log_path.display()))
}

fn from_flags(args: &AddArgs, entries: &[Entry], now: u64) -> Result<Upgrade> {
    let mut files = Vec::new();
    for text in &args.files {
        files.push(notation::file(text)?);
    }
    let mut expected = Vec::new();
    for text in &args.expect {
        expected.push(notation::prediction(text)?);
    }
    Ok(Upgrade {
        id: args
            .id
            .clone()
            .unwrap_or_else(|| upgrades::next_id(entries, now)),
        recorded_at: now,
        title: args.title.clone().unwrap_or_default(),
        contributors: contributors(&args.authors, &args.rationales)?,
        applies_from: AppliesFrom {
            run: args.run.clone(),
            epoch: args.epoch,
        },
        evidence: evidence(&args.saw, &args.metrics, &args.claims)?,
        diagnosis: args.diagnosis.clone(),
        change: Change {
            summary: args.change.clone().unwrap_or_default(),
            files,
            diff: None,
        },
        expected,
    })
}

/// An upgrade written as JSON, with `id` and `recorded_at` filled in when
/// left out.
fn from_file(path: &Path, entries: &[Entry], now: u64) -> Result<Upgrade> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::io("read", path, e))?;
    let decode = |e: serde_json::Error| Error::Store {
        path: path.to_owned(),
        message: e.to_string(),
    };
    let mut value: serde_json::Value = serde_json::from_str(&text).map_err(decode)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| Error::field("--from", "expected a JSON object"))?;
    object.remove("kind");
    object
        .entry("id")
        .or_insert_with(|| upgrades::next_id(entries, now).into());
    object.entry("recorded_at").or_insert_with(|| now.into());
    serde_json::from_value(value).map_err(decode)
}

fn outcome(store: &Store, args: &OutcomeArgs, now: u64, out: &mut dyn Write) -> Result<()> {
    let dir = &args.r#loop;
    let name = loop_name(dir)?;
    let log_path = dir.join(LOG);
    let entries = upgrades::collect(store, &name, upgrades::read(&log_path)?)?;
    let upgrade = upgrades::upgrades(&entries)
        .find(|u| u.id == args.upgrade)
        .ok_or_else(|| Error::Reference {
            kind: "upgrade",
            id: args.upgrade.clone(),
        })?;
    let measured = evidence(&args.saw, &args.metrics, &[])?;
    let verdict = match &args.verdict {
        Some(text) => notation::verdict(text)?,
        None => upgrades::evaluate(&upgrade.expected, &measured).ok_or_else(|| {
            Error::field(
                "--verdict",
                format!(
                    "no prediction of {} was measured; pass --verdict",
                    upgrade.id
                ),
            )
        })?,
    };
    let entry = Entry::Outcome(Outcome {
        upgrade: upgrade.id.clone(),
        recorded_at: now,
        run: args.run.clone(),
        verdict,
        measured,
        contributors: contributors(&args.authors, &args.rationales)?,
    });
    upgrades::append(&log_path, &entry)?;
    let verdict = serde_json::to_string(&verdict).unwrap_or_default();
    say(
        out,
        &format!(
            "recorded outcome of {} in run {}: {}",
            args.upgrade,
            args.run,
            verdict.trim_matches('"')
        ),
    )
}
