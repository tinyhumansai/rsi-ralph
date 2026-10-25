//! A loop's score history across runs and epochs, derived on demand from the
//! run store.
//!
//! Each run's `events.jsonl` already holds every measurement: `measured` at
//! epoch 0 is the baseline (the incumbent the run started from), `measured`
//! at a later epoch is a candidate, and `epoch_started`, `observed` and
//! `epoch_finished` bound each epoch. The run's `ledger.json`, when present,
//! names each candidate's seat and which one was promoted. Nothing new is
//! stored: [`collect`] reads the store each time, so the history is never out
//! of date and covers runs written before it existed.
//!
//! The time between the end of seat work and `epoch_finished` is the
//! epoch's scoring wall time ([`EpochScores::scoring_ms`]): gates, scorers,
//! decisions and promotion, during which every seat is idle. Seat work ends
//! at the epoch's first event after `observed`, which is the first candidate
//! being considered.
//!
//! Upgrades from the [upgrade log](crate::upgrades) are placed on the
//! history: each [`UpgradeMark`] summarizes the run before it and the run it
//! applies to, so its outcome can be read off the scores.

use std::{collections::BTreeMap, fs};

use serde_json::Value;

use crate::{
    Result,
    store::{self, Store},
    upgrades::{self, Entry},
};

pub mod render;
mod types;
pub use types::{EpochScores, RunScores, RunSummary, ScoreHistory, ScorePoint, UpgradeMark};

/// Seat and promotion of each candidate id, from a run's `ledger.json`.
#[derive(Debug, Default)]
struct LedgerFacts {
    seats: BTreeMap<String, String>,
    promoted: Vec<String>,
}

impl LedgerFacts {
    fn from_value(ledger: &Value) -> Self {
        let mut facts = Self::default();
        for c in ledger["candidates"].as_array().into_iter().flatten() {
            let (Some(id), Some(seat)) = (c["id"].as_str(), c["seat"].as_str()) else {
                continue;
            };
            facts.seats.insert(id.to_owned(), seat.to_owned());
            if c["status"] == "promoted" {
                facts.promoted.push(id.to_owned());
            }
        }
        for e in ledger["epochs"].as_array().into_iter().flatten() {
            if let Some(id) = e["promoted"].as_str() {
                facts.promoted.push(id.to_owned());
            }
        }
        facts
    }
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64()
}

fn count(value: &Value) -> u32 {
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0)
}

/// One run's scores from its `events.jsonl` text and, when it has one, its
/// `ledger.json`. Lines that do not decode are skipped.
#[must_use]
pub fn run_scores(run: &str, started_at: u64, events: &str, ledger: Option<&Value>) -> RunScores {
    let facts = ledger.map(LedgerFacts::from_value).unwrap_or_default();
    let mut scores = RunScores {
        run: run.to_owned(),
        started_at,
        maximize: true,
        baseline: None,
        epochs: Vec::new(),
    };
    // Per open epoch: when it started, and when its seats stopped.
    let mut started_ms = 0;
    let mut seats_done: Option<u64> = None;
    let mut improved: Option<f64> = None;
    for line in events.lines() {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let at = record["elapsed_ms"].as_u64().unwrap_or(0);
        let event = &record["event"];
        let kind = event["kind"].as_str().unwrap_or("");
        let epoch = count(&event["epoch"]);
        match kind {
            "run_started" => scores.maximize = event["goal"]["direction"] != "minimize",
            "epoch_started" => {
                started_ms = at;
                seats_done = None;
                improved = None;
                scores.epochs.push(EpochScores {
                    epoch,
                    incumbent: event["incumbent"].as_str().unwrap_or("").to_owned(),
                    incumbent_metric: None,
                    finished_metric: None,
                    candidates: Vec::new(),
                    hive_ms: None,
                    scoring_ms: None,
                });
            }
            "observed" => {
                if let Some(open) = scores.epochs.last_mut() {
                    open.incumbent_metric = number(&event["incumbent_metric"]);
                }
            }
            "measured" if epoch == 0 => {
                scores.baseline = Some(point(event, None, at));
            }
            _ => {}
        }
        let Some(open) = scores.epochs.last_mut().filter(|e| e.epoch == epoch) else {
            continue;
        };
        if !matches!(kind, "epoch_started" | "observed") && seats_done.is_none() {
            seats_done = Some(at);
            open.hive_ms = Some(at.saturating_sub(started_ms));
        }
        match kind {
            "measured" => {
                let id = event["candidate"].as_str().unwrap_or("");
                let seat = facts
                    .seats
                    .get(id)
                    .cloned()
                    .or_else(|| id.strip_prefix(&format!("e{epoch}-")).map(str::to_owned));
                let mut p = point(event, seat, at);
                p.promoted = facts.promoted.iter().any(|c| c == id);
                open.candidates.push(p);
            }
            "improved" => improved = number(&event["metric"]),
            "epoch_finished" => {
                open.finished_metric = number(&event["incumbent_metric"]);
                open.scoring_ms = Some(at.saturating_sub(seats_done.unwrap_or(at)));
                // Without a ledger, the promoted candidate is the first whose
                // mean is the promoted score.
                if ledger.is_none()
                    && let Some(metric) = improved
                    && let Some(p) = open
                        .candidates
                        .iter_mut()
                        .find(|p| p.mean.is_some_and(|m| (m - metric).abs() < 1e-12))
                {
                    p.promoted = true;
                }
            }
            _ => {}
        }
    }
    scores
}

fn point(event: &Value, seat: Option<String>, at: u64) -> ScorePoint {
    ScorePoint {
        candidate: event["candidate"].as_str().unwrap_or("").to_owned(),
        seat,
        mean: number(&event["mean"]),
        failures: count(&event["failures"]),
        seeds: count(&event["seeds"]),
        promoted: false,
        at_ms: at,
    }
}

impl RunScores {
    /// This run in brief.
    #[must_use]
    pub fn summary(&self) -> RunSummary {
        let candidates = self.epochs.iter().flat_map(|e| &e.candidates);
        let best = candidates
            .clone()
            .filter_map(|p| p.mean)
            .reduce(|a, b| if (b > a) == self.maximize { b } else { a });
        RunSummary {
            run: self.run.clone(),
            start: self.baseline.as_ref().and_then(|b| b.mean),
            best,
            end: self.epochs.iter().rev().find_map(|e| e.finished_metric),
            candidates: candidates.clone().count(),
            promoted: candidates.filter(|p| p.promoted).count(),
        }
    }
}

/// Place each upgrade among `runs` (oldest first): the run it applies to and
/// the one before. An upgrade waiting for the next run sits after the last.
#[must_use]
pub fn marks(runs: &[RunScores], entries: &[Entry]) -> Vec<UpgradeMark> {
    let mut carried: BTreeMap<&str, &str> = BTreeMap::new();
    for entry in entries {
        if let Entry::Applied(a) = entry {
            for id in &a.upgrades {
                carried.entry(id.as_str()).or_insert(a.run.as_str());
            }
        }
    }
    upgrades::upgrades(entries)
        .map(|u| {
            let run = u
                .applies_from
                .run
                .as_deref()
                .or_else(|| carried.get(u.id.as_str()).copied());
            let (before, after) = match run {
                Some(run) => match runs.iter().position(|r| r.run == run) {
                    Some(i) => (
                        i.checked_sub(1).map(|j| runs[j].summary()),
                        Some(runs[i].summary()),
                    ),
                    None => (None, None),
                },
                None => (runs.last().map(RunScores::summary), None),
            };
            UpgradeMark {
                id: u.id.clone(),
                title: u.title.clone(),
                run: run.map(str::to_owned),
                before,
                after,
            }
        })
        .collect()
}

/// A loop's score history from every stored run of it, with `entries` (its
/// upgrade log, as [`upgrades::collect`] returns it) placed on it. A run
/// whose trace cannot be read is skipped.
///
/// # Errors
/// Returns [`crate::Error::Io`] when the store cannot be listed.
pub fn collect(store: &Store, loop_name: &str, entries: &[Entry]) -> Result<ScoreHistory> {
    let mut records = store.list_runs(Some(loop_name))?;
    records.reverse();
    let runs: Vec<RunScores> = records
        .iter()
        .filter_map(|record| {
            let dir = store.runs_root().join(loop_name).join(&record.id);
            let events = fs::read_to_string(dir.join("events.jsonl")).ok()?;
            let ledger = store::read_json::<Value>(&dir.join("ledger.json")).ok();
            Some(run_scores(
                &record.id,
                record.started_at,
                &events,
                ledger.as_ref(),
            ))
        })
        .collect();
    Ok(ScoreHistory {
        r#loop: loop_name.to_owned(),
        upgrades: marks(&runs, entries),
        runs,
    })
}

#[cfg(test)]
mod test;
