//! Executes a benchmark matrix and writes its summary.
//!
//! [`plan`] lays the matrix out as [`Job`]s, each with its own run
//! directory `<out>/<benchmark>/<model-label>/<repeat>/`. [`execute`] runs
//! them on up to `jobs` scoped threads, each run with fresh model ports, and
//! returns their records in matrix order. [`write_summary`] writes
//! `summary.json` and `summary.md` beside the run directories.
use super::{RunCell, RunRecord, Summary, model_label, record, render_markdown};
use crate::benchmarks::{Benchmark, Models};
use rsi_ralph::{RunLocation, ScriptLedger};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

/// Which supervisor (and judge) a run uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelChoice {
    /// Each benchmark's canned supervisor and scripted judge.
    Offline,
    /// An `OpenRouter` model, by id, as both supervisor and judge.
    Live(String),
}

impl ModelChoice {
    /// The label used in directories and the table.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Offline => "offline".into(),
            Self::Live(id) => model_label(id),
        }
    }

    /// Fresh model ports for one run of `benchmark`.
    ///
    /// # Errors
    ///
    /// Returns an error when a live model id is invalid.
    pub fn models(&self, benchmark: Benchmark) -> Result<Models, Box<dyn Error>> {
        match self {
            Self::Offline => Ok(benchmark.offline_models()),
            Self::Live(id) => {
                let port = crate::live_model(id)?;
                Ok(Models {
                    supervisor: Arc::clone(&port),
                    judge: Some(port),
                })
            }
        }
    }
}

/// One run of the matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// The benchmark.
    pub benchmark: Benchmark,
    /// The supervisor model.
    pub model: ModelChoice,
    /// Repeat number, from 1.
    pub repeat: u32,
    /// Run id and directory.
    pub run: RunLocation,
}

/// Every benchmark × model × repeat, in that nesting order, under `out`.
#[must_use]
pub fn plan(
    benchmarks: &[Benchmark],
    models: &[ModelChoice],
    repeats: u32,
    out: &Path,
) -> Vec<Job> {
    let mut jobs = vec![];
    for &benchmark in benchmarks {
        for model in models {
            let label = model.label();
            for repeat in 1..=repeats {
                jobs.push(Job {
                    benchmark,
                    model: model.clone(),
                    repeat,
                    run: RunLocation {
                        id: format!("{}-{label}-{repeat}", benchmark.name()),
                        dir: out
                            .join(benchmark.name())
                            .join(&label)
                            .join(repeat.to_string()),
                    },
                });
            }
        }
    }
    jobs
}

/// Runs one job and records it; a failed run is recorded, not returned.
#[must_use]
pub fn run_job(job: &Job, python: &Path) -> RunRecord {
    let start = Instant::now();
    let outcome = job
        .model
        .models(job.benchmark)
        .and_then(|models| job.benchmark.run(&job.run, python, models));
    let wall_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let ledger = std::fs::read(job.run.dir.join("ledger.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ScriptLedger>(&bytes).ok());
    record(
        RunCell {
            benchmark: job.benchmark.name().into(),
            model: job.model.label(),
            repeat: job.repeat,
        },
        ledger.as_ref(),
        wall_ms,
        job.run.dir.clone(),
        outcome.err().map(|e| e.to_string()),
    )
}

/// Runs `jobs` on up to `threads` threads and returns their records in
/// `jobs` order. `on_done` is called as each run finishes.
#[must_use]
pub fn execute(
    jobs: Vec<Job>,
    python: &Path,
    threads: usize,
    on_done: &(dyn Fn(&RunRecord) + Sync),
) -> Vec<RunRecord> {
    let total = jobs.len();
    let queue = Mutex::new(jobs.into_iter().enumerate());
    let done = Mutex::new(Vec::with_capacity(total));
    std::thread::scope(|scope| {
        for _ in 0..threads.clamp(1, total.max(1)) {
            scope.spawn(|| {
                loop {
                    let next = queue.lock().map(|mut q| q.next());
                    let Ok(Some((index, job))) = next else { break };
                    let record = run_job(&job, python);
                    on_done(&record);
                    done.lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push((index, record));
                }
            });
        }
    });
    let mut done = done.into_inner().unwrap_or_else(PoisonError::into_inner);
    done.sort_by_key(|(index, _)| *index);
    done.into_iter().map(|(_, record)| record).collect()
}

/// Writes `summary.json` and `summary.md` (`header`, then the table) into
/// `out`, and returns the paths written.
///
/// # Errors
///
/// Returns an error when `out` cannot be created or written.
pub fn write_summary(
    out: &Path,
    summary: &Summary,
    header: &str,
) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    std::fs::create_dir_all(out)?;
    let json = out.join("summary.json");
    let markdown = out.join("summary.md");
    std::fs::write(&json, serde_json::to_vec_pretty(summary)?)?;
    std::fs::write(
        &markdown,
        format!("{header}\n\n{}", render_markdown(&summary.rows)),
    )?;
    Ok((json, markdown))
}
