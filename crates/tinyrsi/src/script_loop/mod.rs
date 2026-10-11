//! A complete, repeating improvement loop over a plain script.
//!
//! [`run_script_loop`] measures the starting script on fixed seeds (epoch 0),
//! then repeats: observe the incumbent, ask the supervisor for proposals,
//! materialize and execute each candidate on the same seeds, score it paired
//! against the incumbent with [`crate::compare`], and promote the best accepted
//! candidate. It stops when the target is reached, the epoch limit is hit,
//! `patience` epochs pass without a promotion, or the token budget runs out.
//! Every candidate lands in `ledger.json`; every stage is emitted to a
//! [`TraceSink`]. Design, safety limits and file layout are in this module's
//! `README.md` and `docs/specs/script-loop.md`.
mod execute;
mod prompt;
mod score;
mod types;
use crate::{Error, LoopEvent, Result, RunGoal, ScoreDecision, StopReason, TraceSink};
pub use execute::parse_metric;
use execute::{Sources, apply, content_id, materialize, read_sources, run_seed, validate_subject};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use tinyrsi_evolve::{EvolveContext, Evolver, ScriptBrief, ScriptProposal};
use tinyrsi_types::Goal;
pub use types::*;

/// Runs the loop to a stop reason and returns its summary.
///
/// `run.dir` receives `ledger.json`, `candidates/<content-id>/` and `tmp/`.
/// The trace sink receives every [`LoopEvent`]; pass a [`crate::JsonlTrace`]
/// at `run.dir/events.jsonl` for a live view.
///
/// **Safety:** proposals are code that runs locally with the caller's
/// privileges, limited only by a copied workspace, a cleared environment, a
/// timeout and an output cap. See the module README.
///
/// # Errors
///
/// Returns [`Error::InvalidScriptSubject`] for an invalid subject, policy or
/// run location (before anything is written), [`Error::ScriptBaselineFailed`]
/// when the starting script fails on any seed, [`Error::TraceIo`] or
/// [`Error::ArchiveIo`] when the trace or ledger cannot be written, and any
/// error of the supervisor's model port. After `run_started`, a failure still
/// writes `run_finished` with reason `error` and the ledger before returning.
pub fn run_script_loop(
    subject: &ScriptSubject,
    supervisor: &dyn Evolver<ScriptBrief, Proposal = ScriptProposal>,
    policy: &LoopPolicy,
    run: &RunLocation,
    trace: &mut dyn TraceSink,
) -> Result<LoopReport> {
    validate_subject(subject)?;
    validate_policy(policy, run)?;
    let sources = read_sources(subject)?;
    std::fs::create_dir_all(run.dir.join("candidates")).map_err(|_| Error::ArchiveIo)?;
    let skip = std::fs::canonicalize(&run.dir).map_err(|_| Error::ArchiveIo)?;
    let workspace =
        std::fs::canonicalize(&subject.workspace).map_err(|_| Error::InvalidScriptSubject)?;
    let mut state = LoopState {
        subject,
        supervisor,
        policy,
        run,
        workspace,
        skip,
        trace,
        ledger: ScriptLedger {
            run_id: run.id.clone(),
            subject: subject.name.clone(),
            metric: subject.metric.clone(),
            seeds: subject.seeds.clone(),
            tokens: 0,
            stop: None,
            incumbents: vec![],
            candidates: vec![],
        },
        incumbent: Incumbent {
            id: content_id(&sources),
            sources,
            metrics: vec![],
            mean: 0.0,
        },
        epochs: 0,
    };
    state.emit(LoopEvent::RunStarted {
        run_id: run.id.clone(),
        subject: subject.name.clone(),
        goal: RunGoal {
            metric: subject.metric.name.clone(),
            direction: subject.metric.direction,
            target: subject.metric.target,
        },
    })?;
    match state.drive() {
        Ok(reason) => state.finish(reason),
        Err(error) => {
            // Best effort: the original error matters more than a second one.
            let _ = state.finish(StopReason::Error);
            Err(error)
        }
    }
}

fn validate_policy(policy: &LoopPolicy, run: &RunLocation) -> Result<()> {
    // `compare` owns the policy rules; probe it once on a trivial matrix.
    let probe = score::score(
        &[0, 1],
        &[0.0, 0.0],
        &[0.0, 0.0],
        crate::MetricDirection::Maximize,
        &policy.eval,
    );
    if policy.max_epochs == 0
        || policy.proposals_per_epoch == 0
        || policy.patience == 0
        || policy.token_budget == 0
        || probe.is_err()
        || run.id.trim().is_empty()
        || run.id.chars().any(char::is_control)
    {
        return Err(Error::InvalidScriptSubject);
    }
    Ok(())
}

struct Incumbent {
    id: String,
    sources: Sources,
    metrics: Vec<f64>,
    mean: f64,
}

struct LoopState<'a> {
    subject: &'a ScriptSubject,
    supervisor: &'a dyn Evolver<ScriptBrief, Proposal = ScriptProposal>,
    policy: &'a LoopPolicy,
    run: &'a RunLocation,
    workspace: PathBuf,
    skip: PathBuf,
    trace: &'a mut dyn TraceSink,
    ledger: ScriptLedger,
    incumbent: Incumbent,
    epochs: u64,
}

/// What one evaluated candidate produced.
struct Measured {
    runs: Vec<SeedRun>,
    mean: Option<f64>,
    failures: u32,
}

impl LoopState<'_> {
    fn emit(&mut self, event: LoopEvent) -> Result<()> {
        Ok(self.trace.record(event)?)
    }

    fn drive(&mut self) -> Result<StopReason> {
        self.baseline()?;
        if self.reached() {
            return Ok(StopReason::GoalReached);
        }
        let mut stall = 0;
        for epoch in 1..=self.policy.max_epochs {
            if self.ledger.tokens >= self.policy.token_budget {
                return Ok(StopReason::Budget);
            }
            self.epochs = epoch;
            let promoted = self.epoch(epoch)?;
            if self.reached() {
                return Ok(StopReason::GoalReached);
            }
            if self.ledger.tokens >= self.policy.token_budget {
                return Ok(StopReason::Budget);
            }
            stall = if promoted { 0 } else { stall + 1 };
            if stall >= self.policy.patience {
                return Ok(StopReason::Stalled);
            }
        }
        Ok(StopReason::MaxEpochs)
    }

    fn reached(&self) -> bool {
        let metric = &self.subject.metric;
        metric
            .target
            .is_some_and(|t| score::reaches(self.incumbent.mean, t, metric.direction))
    }

    fn baseline(&mut self) -> Result<()> {
        let id = self.incumbent.id.clone();
        let sources = self.incumbent.sources.clone();
        let measured = self.measure(0, &id, &sources)?;
        let ok = measured.failures == 0;
        self.ledger.candidates.push(LedgerCandidate {
            epoch: 0,
            proposal_id: None,
            id: Some(id.clone()),
            parent: None,
            rationale: "baseline".into(),
            hypothesis: "starting script".into(),
            paths: vec![],
            mean: measured.mean,
            decision: CandidateDecision::Baseline,
            estimate: None,
            lower: None,
            upper: None,
            promoted: ok,
            note: None,
            runs: measured.runs,
        });
        let metrics = self.ledger.candidates[0]
            .runs
            .iter()
            .filter_map(|r| r.metric)
            .collect::<Vec<_>>();
        if let (true, Some(mean)) = (ok, measured.mean) {
            self.incumbent.metrics = metrics;
            self.incumbent.mean = mean;
            self.ledger.incumbents.push(IncumbentEntry {
                epoch: 0,
                id,
                metric: mean,
            });
            self.save()
        } else {
            self.save()?;
            Err(Error::ScriptBaselineFailed)
        }
    }

    /// Runs one epoch and reports whether it promoted a candidate.
    fn epoch(&mut self, epoch: u64) -> Result<bool> {
        let parent = self.incumbent.id.clone();
        self.emit(LoopEvent::EpochStarted {
            epoch,
            incumbent: parent.clone(),
        })?;
        let rejected = self
            .ledger
            .candidates
            .iter()
            .filter(|c| c.proposal_id.is_some() && c.decision != CandidateDecision::Accept)
            .count();
        self.emit(LoopEvent::Observed {
            epoch,
            incumbent: parent.clone(),
            incumbent_metric: Some(self.incumbent.mean),
            history: count(self.ledger.incumbents.len()),
            rejected: count(rejected),
        })?;
        let mut accepted: Vec<(usize, Sources)> = vec![];
        for k in 0..self.policy.proposals_per_epoch {
            if self.ledger.tokens >= self.policy.token_budget {
                break;
            }
            if let Some(found) = self.propose(epoch, k, &parent)? {
                accepted.push(found);
            }
        }
        let direction = self.subject.metric.direction;
        let best = accepted.into_iter().reduce(|a, b| {
            match (
                self.ledger.candidates[a.0].mean,
                self.ledger.candidates[b.0].mean,
            ) {
                (Some(x), Some(y)) if score::better(y, x, direction) => b,
                _ => a,
            }
        });
        let promoted = best.is_some();
        if let Some((row, sources)) = best {
            self.promote(epoch, row, sources)?;
        }
        self.emit(LoopEvent::EpochFinished {
            epoch,
            incumbent: self.incumbent.id.clone(),
            incumbent_metric: Some(self.incumbent.mean),
        })?;
        Ok(promoted)
    }

    /// Requests, executes and scores one proposal; returns its ledger row when
    /// it was accepted.
    fn propose(&mut self, epoch: u64, k: u32, parent: &str) -> Result<Option<(usize, Sources)>> {
        let proposal_id = format!("p{epoch}-{k}");
        let checked = self.ask(epoch, k)?;
        let mut row = LedgerCandidate {
            epoch,
            proposal_id: Some(proposal_id.clone()),
            id: None,
            parent: Some(parent.to_owned()),
            rationale: String::new(),
            hypothesis: String::new(),
            paths: vec![],
            runs: vec![],
            mean: None,
            decision: CandidateDecision::Invalid,
            estimate: None,
            lower: None,
            upper: None,
            promoted: false,
            note: None,
        };
        let proposal = match checked {
            Ok(proposal) => proposal,
            Err(error) => {
                row.note = Some(error.to_string());
                self.emit(LoopEvent::Ideated {
                    epoch,
                    proposal_id,
                    parent: parent.to_owned(),
                    candidate: None,
                    rationale: format!("invalid proposal: {error}"),
                    hypothesis: String::new(),
                })?;
                self.ledger.candidates.push(row);
                self.save()?;
                return Ok(None);
            }
        };
        row.rationale.clone_from(&proposal.rationale);
        row.hypothesis.clone_from(&proposal.hypothesis);
        row.paths = proposal.files.iter().map(|f| f.path.clone()).collect();
        let sources = apply(&proposal, &self.incumbent.sources);
        let id = content_id(&sources);
        self.emit(LoopEvent::Ideated {
            epoch,
            proposal_id,
            parent: parent.to_owned(),
            candidate: Some(id.clone()),
            rationale: proposal.rationale.clone(),
            hypothesis: proposal.hypothesis.clone(),
        })?;
        row.id = Some(id.clone());
        if self
            .ledger
            .candidates
            .iter()
            .any(|c| c.id.as_deref() == Some(&id))
        {
            row.decision = CandidateDecision::Duplicate;
            row.note = Some("content already evaluated in this run".into());
            self.scored(epoch, &id, ScoreDecision::Reject, (0.0, 0.0, 0.0))?;
            self.ledger.candidates.push(row);
            self.save()?;
            return Ok(None);
        }
        let measured = self.measure(epoch, &id, &sources)?;
        let (decision, interval) = self.judge(&measured)?;
        self.scored(epoch, &id, decision, interval)?;
        row.runs = measured.runs;
        row.mean = measured.mean;
        row.decision = match decision {
            ScoreDecision::Accept => CandidateDecision::Accept,
            ScoreDecision::Reject => CandidateDecision::Reject,
            ScoreDecision::Inconclusive => CandidateDecision::Inconclusive,
        };
        (row.estimate, row.lower, row.upper) =
            (Some(interval.0), Some(interval.1), Some(interval.2));
        self.ledger.candidates.push(row);
        self.save()?;
        Ok(
            (decision == ScoreDecision::Accept)
                .then(|| (self.ledger.candidates.len() - 1, sources)),
        )
    }

    /// Asks the evolver for one proposal, charging its measured tokens. The
    /// inner result is the proposal or the reason it was rejected; the outer
    /// error is a failed evolver call, which ends the run.
    fn ask(&mut self, epoch: u64, k: u32) -> Result<Result<ScriptProposal>> {
        let brief = ScriptBrief {
            prompt: prompt::prompt(
                self.subject,
                &self.ledger,
                &self.incumbent.sources,
                self.policy.rejected_context,
            ),
            editable: self.subject.editable.clone(),
        };
        let context = EvolveContext {
            epoch,
            index: k,
            seed: (epoch << 16) | u64::from(k),
        };
        let proposed =
            self.supervisor
                .propose(&brief, &Goal::new(self.subject.brief.clone()), &context)?;
        self.ledger.tokens = self.ledger.tokens.saturating_add(proposed.tokens);
        // The script loop asks for one proposal per call: none, or a batch
        // it would have to silently truncate, is malformed. Every proposal is
        // validated here, whichever evolver produced it, before `apply` can
        // write a path.
        let mut proposals = proposed.proposals.into_iter();
        let proposal = match (proposals.next(), proposals.next()) {
            (Some(only), None) => only,
            _ => Err(tinyrsi_evolve::Error::SupervisorMalformed),
        };
        Ok(proposal
            .and_then(|p| p.validate(&self.subject.editable).map(|()| p))
            .map_err(Error::from))
    }

    /// Scores a measured candidate; any failed seed is a rejection.
    fn judge(&self, measured: &Measured) -> Result<(ScoreDecision, (f64, f64, f64))> {
        let direction = self.subject.metric.direction;
        if measured.failures == 0 {
            let metrics: Vec<f64> = measured.runs.iter().filter_map(|r| r.metric).collect();
            let s = score::score(
                &self.subject.seeds,
                &self.incumbent.metrics,
                &metrics,
                direction,
                &self.policy.eval,
            )?;
            return Ok((s.decision, (s.estimate, s.lower, s.upper)));
        }
        let base: Vec<Option<f64>> = self.incumbent.metrics.iter().copied().map(Some).collect();
        let cand: Vec<Option<f64>> = measured.runs.iter().map(|r| r.metric).collect();
        let multiplier = self.policy.eval.standard_error_multiplier;
        Ok((
            ScoreDecision::Reject,
            score::interval(&base, &cand, direction, multiplier),
        ))
    }

    fn scored(
        &mut self,
        epoch: u64,
        id: &str,
        decision: ScoreDecision,
        (estimate, lower, upper): (f64, f64, f64),
    ) -> Result<()> {
        self.emit(LoopEvent::Scored {
            epoch,
            candidate: id.to_owned(),
            decision,
            estimate,
            lower,
            upper,
            units: tinyrsi_trace::IntervalUnits::Metric,
        })
    }

    fn promote(&mut self, epoch: u64, row: usize, sources: Sources) -> Result<()> {
        let candidate = &mut self.ledger.candidates[row];
        candidate.promoted = true;
        let id = candidate.id.clone().ok_or(Error::InvalidRecord)?;
        let mean = candidate.mean.ok_or(Error::InvalidRecord)?;
        let metrics: Vec<f64> = candidate.runs.iter().filter_map(|r| r.metric).collect();
        let from = std::mem::replace(
            &mut self.incumbent,
            Incumbent {
                id: id.clone(),
                sources,
                metrics,
                mean,
            },
        )
        .id;
        self.ledger.incumbents.push(IncumbentEntry {
            epoch,
            id: id.clone(),
            metric: mean,
        });
        self.emit(LoopEvent::Improved {
            epoch,
            from,
            to: id,
            metric: mean,
        })?;
        self.save()
    }

    fn candidate_dir(&self, id: &str) -> PathBuf {
        self.run.dir.join("candidates").join(id)
    }

    fn measure(&mut self, epoch: u64, id: &str, sources: &Sources) -> Result<Measured> {
        let dir = self.candidate_dir(id);
        materialize(&self.workspace, &self.skip, &dir, sources)?;
        let tmp = self.run.dir.join("tmp");
        let mut runs = vec![];
        for &seed in &self.subject.seeds {
            let run = run_seed(self.subject, &dir, &tmp, seed);
            self.emit(LoopEvent::Executed {
                epoch,
                candidate: id.to_owned(),
                seed,
                metric: run.metric,
                ok: run.metric.is_some(),
                duration_ms: run.duration_ms,
            })?;
            runs.push(run);
        }
        let metrics: Vec<f64> = runs.iter().filter_map(|r| r.metric).collect();
        let failures = count(runs.len() - metrics.len());
        let mean = (!metrics.is_empty())
            .then(|| metrics.iter().sum::<f64>() / metrics.iter().fold(0.0, |n, _| n + 1.0));
        self.emit(LoopEvent::Measured {
            epoch,
            candidate: id.to_owned(),
            mean,
            failures,
            seeds: count(runs.len()),
        })?;
        Ok(Measured {
            runs,
            mean,
            failures,
        })
    }

    fn finish(&mut self, reason: StopReason) -> Result<LoopReport> {
        self.ledger.stop = Some(reason);
        let measured = !self.ledger.incumbents.is_empty();
        self.emit(LoopEvent::RunFinished {
            reason,
            incumbent: self.incumbent.id.clone(),
            incumbent_metric: measured.then_some(self.incumbent.mean),
        })?;
        self.save()?;
        let rejected = self
            .ledger
            .candidates
            .iter()
            .filter(|c| c.proposal_id.is_some() && c.decision != CandidateDecision::Accept)
            .count();
        let baseline = self
            .ledger
            .incumbents
            .first()
            .ok_or(Error::ScriptBaselineFailed)?;
        Ok(LoopReport {
            run_id: self.run.id.clone(),
            reason,
            baseline: baseline.id.clone(),
            baseline_metric: baseline.metric,
            incumbent: self.incumbent.id.clone(),
            incumbent_metric: self.incumbent.mean,
            epochs: self.epochs,
            promotions: count(self.ledger.incumbents.len().saturating_sub(1)),
            rejected: count(rejected),
            tokens: self.ledger.tokens,
            ledger: self.ledger_path(),
        })
    }

    fn ledger_path(&self) -> PathBuf {
        self.run.dir.join("ledger.json")
    }

    fn save(&self) -> Result<()> {
        write_atomic(&self.ledger_path(), &self.ledger)
    }
}

/// Saturating `usize` to `u32` for trace counters.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Replaces `path` with `value` as pretty JSON: temporary file, `fsync`,
/// rename, then directory `fsync` — the same protocol the archive uses.
fn write_atomic(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let parent = path.parent().ok_or(Error::ArchiveIo)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| Error::ArchiveIo)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|_| Error::ArchiveIo)?;
    temp.write_all(&bytes).map_err(|_| Error::ArchiveIo)?;
    temp.as_file().sync_all().map_err(|_| Error::ArchiveIo)?;
    temp.persist(path).map_err(|_| Error::ArchiveIo)?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::ArchiveIo)?;
    Ok(())
}

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_loop;
