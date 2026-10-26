//! The epoch loop.
//!
//! One run: score the incumbent (or reuse a cached score for the same
//! revision and scorers), then per epoch render the brief, optionally ask the
//! retreat for ideas, give each editing seat its own candidate checkout, run
//! the hive, and for every changed candidate run the surface check, the
//! gates, the scorers and the decision. The best accepted candidate is
//! promoted. With `carry`, every candidate's notes are then harvested into
//! the store and shown in the next brief. The run stops on `goal_reached`, `stalled`, `budget`,
//! `max_epochs`, `provider_refused` or `error`, and every step is mirrored
//! to `events.jsonl`, `ledger.json` and `run.json` as it happens.
//!
//! An epoch whose hive changed nothing while the model provider refused its
//! calls is not an ordinary empty epoch: see the `refusal` module.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use rsi_ralph_score::ScoreDecision;
use rsi_ralph_trace::{JsonlTrace, LoopEvent, MonotonicClock, RunGoal, StopReason, TraceSink};
use sha2::{Digest, Sha256};

use crate::{
    Error, Result,
    brief::{self, BriefInput, HistoryEntry},
    carry::{self, EpochNotes, Source},
    gate,
    manifest::{DecideTest, Loop},
    score::{self, Judges, Scorecard},
    step::StepContext,
    store::{self, RunDir, RunRecord, RunStatus, Store},
    subject::{Checkout, Subject},
};

mod refusal;
mod types;
pub use types::{
    CandidateRecord, CandidateStatus, EpochRecord, Ideator, Ledger, Meter, ProposalReport,
    ProposalRequest, Proposer,
};

/// Everything one run needs besides the loop itself.
pub struct Engine<'a> {
    /// The parsed loop.
    pub parsed: &'a Loop,
    /// The loop folder on disk; steps run here.
    pub loop_dir: PathBuf,
    /// The run store.
    pub store: &'a Store,
    /// This run's folder.
    pub run: RunDir,
    /// The hive.
    pub proposer: &'a mut dyn Proposer,
    /// The creative retreat, when enabled.
    pub ideator: Option<&'a mut dyn Ideator>,
    /// Judges by scorer id.
    pub judges: Judges,
    /// Spend so far, when model calls are metered.
    pub meter: Option<&'a dyn Meter>,
    /// Base URL for step model calls.
    pub model_url: Option<String>,
    /// `PATH` and `HOME` for steps.
    pub inherited: Vec<(String, String)>,
    /// Key values to scrub from stored errors.
    pub secrets: Vec<String>,
    /// Epoch limit overriding the manifest's.
    pub epochs: Option<u32>,
    /// Problems found before the run started, such as a hive change no
    /// upgrade record explains; they open `run.json`'s `warnings`.
    pub warnings: Vec<String>,
}

impl std::fmt::Debug for Engine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("loop", &self.parsed.manifest.name)
            .field("run", &self.run.id)
            .finish_non_exhaustive()
    }
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSummary {
    /// Why it stopped.
    pub reason: StopReason,
    /// The final incumbent's revision.
    pub incumbent: String,
    /// The final incumbent's deciding score.
    pub score: Option<f64>,
    /// Where the final incumbent is checked out.
    pub checkout: PathBuf,
    /// Candidates promoted during the run.
    pub promotions: usize,
    /// What stopped a run that ended on `provider_refused`.
    pub error: Option<String>,
    /// Problems that did not stop the run, as in `run.json`.
    pub warnings: Vec<String>,
}

/// What one epoch did.
struct EpochOutcome {
    /// A candidate was promoted.
    promoted: bool,
    /// The provider's refusals explain the epoch.
    refused: Option<refusal::Refused>,
}

/// Mutable state of one run.
struct State {
    trace: JsonlTrace,
    record: RunRecord,
    ledger: Ledger,
    incumbent: Checkout,
    revision: String,
    card: Scorecard,
    /// The last epoch's harvested notes and its folder, for `{{notes}}`.
    notes: Option<(PathBuf, EpochNotes)>,
}

impl Engine<'_> {
    /// Run the loop to a stop reason.
    ///
    /// The run's `run.json` is marked failed, with the error, if the loop
    /// errors.
    ///
    /// # Errors
    /// Returns the first error that stopped the run.
    pub fn run(mut self) -> Result<RunSummary> {
        let mut record = RunRecord {
            id: self.run.id.clone(),
            r#loop: self.parsed.manifest.name.clone(),
            status: RunStatus::Running,
            pid: std::process::id(),
            started_at: store::now(),
            finished_at: None,
            stop_reason: None,
            incumbent: String::new(),
            incumbent_score: None,
            epochs: 0,
            spend_usd: 0.0,
            error: None,
            warnings: std::mem::take(&mut self.warnings),
        };
        store::write_json(&self.run.record_path(), &record)?;
        let result = self.execute(&mut record);
        record.finished_at = Some(store::now());
        record.spend_usd = self.meter.map_or(0.0, Meter::spent_usd);
        match &result {
            Ok(summary) => {
                // A provider that refuses every call is a failure the
                // operator must fix, not a finished search.
                record.status = if summary.reason == StopReason::ProviderRefused {
                    RunStatus::Failed
                } else {
                    RunStatus::Finished
                };
                record.stop_reason = Some(summary.reason);
            }
            Err(e) => {
                record.status = RunStatus::Failed;
                record.stop_reason = Some(StopReason::Error);
                record.error = Some(store::scrub(
                    &crate::error::chain(e),
                    self.secrets.iter().map(String::as_str),
                ));
            }
        }
        store::write_json(&self.run.record_path(), &record)?;
        result
    }

    fn execute(&mut self, record: &mut RunRecord) -> Result<RunSummary> {
        let manifest = &self.parsed.manifest;
        let spec = &manifest.score;
        let mut trace = JsonlTrace::create(self.run.events_path(), MonotonicClock::new())?;
        trace.record(LoopEvent::RunStarted {
            run_id: self.run.id.clone(),
            subject: manifest.name.clone(),
            goal: RunGoal {
                metric: spec.decide.on.clone(),
                direction: match spec.decide.test {
                    DecideTest::Seeds => spec.decide.direction,
                    DecideTest::Sign => rsi_ralph_score::MetricDirection::Maximize,
                },
                target: spec.decide.target,
            },
        })?;
        let subject = Subject::from_spec(&manifest.subject, &self.loop_dir);
        let incumbent = subject.incumbent(&self.run.path)?;
        let revision = subject.revision(&incumbent)?;
        let card = self.baseline(&incumbent, &revision, &mut trace)?;
        record.incumbent.clone_from(&revision);
        record.incumbent_score = card.headline;
        store::write_json(&self.run.record_path(), &*record)?;
        let mut state = State {
            trace,
            record: record.clone(),
            ledger: Ledger {
                incumbents: vec![revision.clone()],
                incumbent: Some(card.clone()),
                ..Ledger::default()
            },
            incumbent,
            revision,
            card,
            notes: None,
        };
        let reason = self.epochs(&subject, &mut state)?;
        state.trace.record(LoopEvent::RunFinished {
            reason,
            incumbent: state.revision.clone(),
            incumbent_metric: state.card.headline,
        })?;
        *record = state.record;
        Ok(RunSummary {
            reason,
            incumbent: state.revision,
            score: state.card.headline,
            checkout: state.incumbent.path,
            promotions: state.ledger.incumbents.len() - 1,
            error: record.error.clone(),
            warnings: record.warnings.clone(),
        })
    }

    fn context(&self, epoch: u32, incumbent: &Checkout) -> StepContext {
        StepContext {
            loop_dir: self.loop_dir.clone(),
            run_dir: self.run.path.clone(),
            incumbent: incumbent.path.clone(),
            epoch,
            model_url: self.model_url.clone(),
            env: self.parsed.manifest.env.clone(),
            inherited: self.inherited.clone(),
        }
    }

    /// Score the incumbent, or reuse the cached score of the same revision
    /// under the same scorers and step environment (`env` can change what a
    /// scorer measures, such as how many items it scores).
    fn baseline(
        &self,
        incumbent: &Checkout,
        revision: &str,
        trace: &mut JsonlTrace,
    ) -> Result<Scorecard> {
        let spec = &self.parsed.manifest.score;
        let key = {
            let mut hasher = Sha256::new();
            hasher.update(revision.as_bytes());
            hasher.update(serde_json::to_vec(spec).unwrap_or_default());
            hasher.update(serde_json::to_vec(&self.parsed.manifest.env).unwrap_or_default());
            crate::subject::hex(&hasher.finalize()[..8])
        };
        let cached = self
            .store
            .cache_dir(&self.parsed.manifest.name)
            .join(format!("{key}.json"));
        let card = match store::read_json::<Scorecard>(&cached) {
            Ok(card) if card.headline.is_some() => card,
            _ => {
                let dir = self.run.epoch_dir(0)?.join("incumbent");
                let card = score::score_checkout(
                    &self.context(0, incumbent),
                    spec,
                    incumbent,
                    &dir,
                    &self.judges,
                    &self.parsed.rubrics,
                    None,
                )?;
                if card.headline.is_some() {
                    store::write_json(&cached, &card)?;
                }
                card
            }
        };
        emit_measurements(trace, 0, revision, &card, &spec.decide.on)?;
        if card.headline.is_none() {
            let reason = card
                .runs
                .get(&spec.decide.on)
                .and_then(|runs| runs.iter().find_map(|r| r.reason.clone()))
                .unwrap_or_else(|| "no score".into());
            return Err(Error::Step {
                step: format!("score.{}", spec.decide.on),
                reason: format!("the incumbent could not be scored: {reason}"),
            });
        }
        Ok(card)
    }

    fn epochs(&mut self, subject: &Subject, state: &mut State) -> Result<StopReason> {
        let manifest = &self.parsed.manifest;
        let spec = &manifest.score;
        let max = self.epochs.unwrap_or(manifest.epochs);
        let mut stalled = 0;
        if state.card.headline.is_some_and(|h| score::reaches(spec, h)) {
            return Ok(StopReason::GoalReached);
        }
        for epoch in 1..=max {
            if !self.affordable() {
                return Ok(StopReason::Budget);
            }
            state.record.epochs = epoch;
            self.save(state)?;
            let outcome = self.epoch(epoch, subject, state)?;
            if let Some(ideator) = self.ideator.as_deref_mut() {
                ideator.outcome(epoch, outcome.promoted)?;
            }
            stalled = if outcome.promoted { 0 } else { stalled + 1 };
            state.trace.record(LoopEvent::EpochFinished {
                epoch: epoch.into(),
                incumbent: state.revision.clone(),
                incumbent_metric: state.card.headline,
            })?;
            if let Some(refused) = outcome.refused {
                if refused.all {
                    state.record.error = Some(refused.line);
                    self.save(state)?;
                    return Ok(StopReason::ProviderRefused);
                }
                state
                    .record
                    .warnings
                    .push(format!("epoch {epoch}: {}", refused.line));
            }
            self.save(state)?;
            if state.card.headline.is_some_and(|h| score::reaches(spec, h)) {
                return Ok(StopReason::GoalReached);
            }
            if stalled >= manifest.patience {
                return Ok(StopReason::Stalled);
            }
        }
        Ok(StopReason::MaxEpochs)
    }

    /// Whether the score reserve can still be spent.
    fn affordable(&self) -> bool {
        let reserve = self.parsed.manifest.budget.score_reserve_usd;
        self.meter
            .and_then(Meter::remaining_usd)
            .is_none_or(|remaining| remaining > reserve)
    }

    fn save(&self, state: &mut State) -> Result<()> {
        state.record.incumbent.clone_from(&state.revision);
        state.record.incumbent_score = state.card.headline;
        state.record.spend_usd = self.meter.map_or(0.0, Meter::spent_usd);
        store::write_json(&self.run.record_path(), &state.record)?;
        store::write_json(&self.run.ledger_path(), &state.ledger)
    }

    /// One epoch: whether a candidate was promoted, and whether provider
    /// refusals explain it.
    fn epoch(&mut self, epoch: u32, subject: &Subject, state: &mut State) -> Result<EpochOutcome> {
        let manifest = &self.parsed.manifest;
        let dir = self.run.epoch_dir(epoch)?;
        state.trace.record(LoopEvent::EpochStarted {
            epoch: epoch.into(),
            incumbent: state.revision.clone(),
        })?;
        let history = history(&state.ledger);
        let rejected = state
            .ledger
            .candidates
            .iter()
            .filter(|c| c.status != CandidateStatus::Promoted)
            .count();
        state.trace.record(LoopEvent::Observed {
            epoch: epoch.into(),
            incumbent: state.revision.clone(),
            incumbent_metric: state.card.headline,
            history: u32::try_from(state.ledger.candidates.len()).unwrap_or(u32::MAX),
            rejected: u32::try_from(rejected).unwrap_or(u32::MAX),
        })?;
        let notes = match (&manifest.carry, &state.notes) {
            (Some(spec), Some((from, notes))) => carry::render(from, notes, spec.brief_chars),
            _ => String::new(),
        };
        let input = |ideas: &'_ [String]| {
            brief::render(
                &self.parsed.brief,
                &BriefInput {
                    incumbent: &state.card,
                    decide_on: &manifest.score.decide.on,
                    history: &history,
                    ideas,
                    notes: &notes,
                },
            )
        };
        let ideas = match self.ideator.as_deref_mut() {
            Some(ideator) => ideator.ideas(epoch, &input(&[]), &dir)?,
            None => Vec::new(),
        };
        let text = input(&ideas);
        store::write_atomic(&dir.join("brief.md"), text.as_bytes())?;

        let mut candidates = BTreeMap::new();
        for seat in self.proposer.editing_seats() {
            let label = format!("e{epoch}-{seat}");
            let branch = format!("rsi/cand-{}-{label}", self.run.id);
            let checkout = subject.candidate(
                &state.incumbent,
                &label,
                &branch,
                &dir.join("candidates").join(&seat),
            )?;
            candidates.insert(seat, checkout);
        }
        let mark = self.meter.map(Meter::call_mark).unwrap_or_default();
        let report = self.proposer.propose(&ProposalRequest {
            epoch,
            brief: &text,
            incumbent: &state.incumbent,
            candidates: &candidates,
            dir: &dir.join("hive"),
        })?;
        let calls = self
            .meter
            .map(|m| m.upstream_calls_since(mark))
            .unwrap_or_default();
        state.ledger.epochs.push(EpochRecord {
            epoch,
            incumbent: state.revision.clone(),
            report,
            ideas,
            promoted: None,
        });

        let first = state.ledger.candidates.len();
        let accepted = self.consider_all(epoch, &candidates, subject, state, &dir)?;
        let changed = state.ledger.candidates[first..]
            .iter()
            .any(|c| !c.changed.is_empty());
        let refused = refusal::explain(
            epoch,
            &calls,
            changed,
            self.secrets.iter().map(String::as_str),
        );
        if let Some(refused) = &refused {
            state.trace.record(refused.event.clone())?;
        }
        let promoted = self.promote_best(epoch, subject, state, &candidates, accepted)?;
        if let Some(spec) = &manifest.carry {
            let notes = harvest(spec, epoch, &state.ledger, first, &dir)?;
            state.notes = Some((dir, notes));
        }
        Ok(EpochOutcome { promoted, refused })
    }

    /// Promote the best accepted candidate, if any; returns whether one was.
    fn promote_best(
        &self,
        epoch: u32,
        subject: &Subject,
        state: &mut State,
        candidates: &BTreeMap<String, Checkout>,
        accepted: Vec<usize>,
    ) -> Result<bool> {
        let manifest = &self.parsed.manifest;
        let best = accepted.into_iter().reduce(|a, b| {
            let score = |i: usize| {
                state.ledger.candidates[i]
                    .scorecard
                    .as_ref()
                    .and_then(|c| c.headline)
            };
            match (score(a), score(b)) {
                (Some(x), Some(y)) if score::better(&manifest.score, y, x) => b,
                _ => a,
            }
        });
        let Some(best) = best else {
            return Ok(false);
        };
        let winner = state.ledger.candidates[best].clone();
        let checkout = &candidates[&winner.seat];
        subject.promote(&state.incumbent, checkout)?;
        let revision = subject.revision(&state.incumbent)?;
        let card = winner
            .scorecard
            .clone()
            .unwrap_or_else(|| state.card.clone());
        state.trace.record(LoopEvent::Improved {
            epoch: epoch.into(),
            from: state.revision.clone(),
            to: revision.clone(),
            metric: card.headline.unwrap_or_default(),
        })?;
        state.ledger.candidates[best].status = CandidateStatus::Promoted;
        if let Some(last) = state.ledger.epochs.last_mut() {
            last.promoted = Some(winner.id.clone());
        }
        state.ledger.incumbents.push(revision.clone());
        state.ledger.incumbent = Some(card.clone());
        state.revision = revision;
        state.card = card;
        Ok(true)
    }

    /// Gate, score and decide every candidate of an epoch, in seat order.
    /// Returns the ledger indices of the accepted ones.
    ///
    /// With `score.parallel_candidates` of 1 each candidate is gated, scored
    /// and decided before the next one starts. With more, every candidate is
    /// gated first (still one after another), up to that many are then scored
    /// at once, and the decisions are made afterwards in seat order against
    /// the same incumbent, so the ledger, the verdicts and the winner are
    /// those of the sequential path; only the trace differs, in that every
    /// `ideated` precedes the first `executed` of the epoch.
    fn consider_all(
        &mut self,
        epoch: u32,
        candidates: &BTreeMap<String, Checkout>,
        subject: &Subject,
        state: &mut State,
        dir: &Path,
    ) -> Result<Vec<usize>> {
        let limit =
            usize::try_from(self.parsed.manifest.score.parallel_candidates).unwrap_or(usize::MAX);
        let mut accepted = Vec::new();
        if limit <= 1 {
            for (seat, checkout) in candidates {
                let mut record = new_record(epoch, seat, checkout);
                let outcome = self
                    .prepare(epoch, checkout, subject, state, dir, &mut record)
                    .and_then(|job| match job {
                        None => Ok(()),
                        Some(job) => {
                            let ctx = self.context(epoch, &state.incumbent);
                            let card = self.score(&ctx, checkout, &job)?;
                            self.finish(epoch, state, checkout, &mut record, card)
                        }
                    });
                accepted.extend(self.settle(state, record, outcome));
                self.save(state)?;
            }
            return Ok(accepted);
        }
        let mut pending = Vec::new();
        for (seat, checkout) in candidates {
            let mut record = new_record(epoch, seat, checkout);
            let job = self.prepare(epoch, checkout, subject, state, dir, &mut record);
            pending.push((checkout, record, job));
        }
        let ctx = self.context(epoch, &state.incumbent);
        let cards = self.score_concurrently(&ctx, limit, &pending);
        for ((checkout, mut record, job), card) in pending.into_iter().zip(cards) {
            let outcome = match (job, card) {
                (Err(e), _) | (Ok(Some(_)), Some(Err(e))) => Err(e),
                (Ok(None), _) | (Ok(Some(_)), None) => Ok(()),
                (Ok(Some(_)), Some(Ok(None))) => {
                    record.status = CandidateStatus::Unaffordable;
                    Ok(())
                }
                (Ok(Some(_)), Some(Ok(Some(card)))) => {
                    self.finish(epoch, state, checkout, &mut record, card)
                }
            };
            accepted.extend(self.settle(state, record, outcome));
            self.save(state)?;
        }
        Ok(accepted)
    }

    /// Record a candidate's outcome in the ledger. Returns its index when it
    /// was accepted.
    fn settle(
        &self,
        state: &mut State,
        mut record: CandidateRecord,
        outcome: Result<()>,
    ) -> Option<usize> {
        if let Err(e) = outcome {
            record.status = CandidateStatus::Error;
            record.error = Some(store::scrub(
                &crate::error::chain(&e),
                self.secrets.iter().map(String::as_str),
            ));
        }
        let accepted = record.status == CandidateStatus::Scored
            && record
                .verdict
                .as_ref()
                .is_some_and(|v| v.decision == ScoreDecision::Accept);
        state.ledger.candidates.push(record);
        accepted.then(|| state.ledger.candidates.len() - 1)
    }

    /// Everything before scoring: the change, the gates and the budget.
    /// Returns what scoring needs, or `None` when the candidate is settled
    /// without being scored (empty, gated or unaffordable).
    fn prepare(
        &self,
        epoch: u32,
        checkout: &Checkout,
        subject: &Subject,
        state: &mut State,
        dir: &Path,
        record: &mut CandidateRecord,
    ) -> Result<Option<ScoreJob>> {
        let manifest = &self.parsed.manifest;
        record.changed = subject.changes(&state.incumbent, checkout)?;
        if record.changed.is_empty() {
            return Ok(None);
        }
        state.trace.record(LoopEvent::Ideated {
            epoch: epoch.into(),
            proposal_id: checkout.label.clone(),
            parent: state.revision.clone(),
            candidate: Some(checkout.label.clone()),
            rationale: format!("{} changed {} files", record.seat, record.changed.len()),
            hypothesis: record.changed.join(", "),
            approach: None,
        })?;
        let steps = dir.join("steps").join(&checkout.label);
        let ctx = self.context(epoch, &state.incumbent);
        record
            .gates
            .push(gate::surface(&manifest.subject, &record.changed));
        if record.gates.iter().all(|g| g.ok) {
            record
                .gates
                .extend(gate::run_gates(&ctx, &manifest.gate, checkout, &steps)?);
        }
        if let Some(failed) = record.gates.iter().find(|g| !g.ok) {
            record.status = CandidateStatus::Gated;
            state.trace.record(LoopEvent::Scored {
                epoch: epoch.into(),
                candidate: checkout.label.clone(),
                decision: ScoreDecision::Reject,
                estimate: 0.0,
                lower: 0.0,
                upper: 0.0,
                units: rsi_ralph_trace::IntervalUnits::Quality,
            })?;
            record.error.clone_from(&failed.reason);
            return Ok(None);
        }
        if !self.affordable() {
            record.status = CandidateStatus::Unaffordable;
            return Ok(None);
        }
        let diff = subject.diff(&state.incumbent, checkout)?;
        store::write_atomic(&steps.join("change.diff"), diff.as_bytes())?;
        Ok(Some(ScoreJob { steps, diff }))
    }

    /// Score one prepared candidate.
    fn score(&self, ctx: &StepContext, checkout: &Checkout, job: &ScoreJob) -> Result<Scorecard> {
        score_job(self.parsed, &self.judges, ctx, checkout, job)
    }

    /// Score the prepared candidates, up to `limit` at a time, taking them in
    /// order. A result is `None` for a candidate with nothing to score, and
    /// `Ok(None)` for one the budget reserve no longer covered when its turn
    /// came. Scorers that call the model do so through the run's gateway,
    /// which meters and caps the spend of all of them together.
    fn score_concurrently(
        &self,
        ctx: &StepContext,
        limit: usize,
        pending: &[(&Checkout, CandidateRecord, Result<Option<ScoreJob>>)],
    ) -> Vec<Option<Result<Option<Scorecard>>>> {
        let next = AtomicUsize::new(0);
        let slots: Vec<Mutex<Option<Result<Option<Scorecard>>>>> =
            pending.iter().map(|_| Mutex::new(None)).collect();
        let (parsed, judges, meter) = (self.parsed, &self.judges, self.meter);
        thread::scope(|scope| {
            for _ in 0..limit.min(pending.len()) {
                scope.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some((checkout, _, job)) = pending.get(i) else {
                            break;
                        };
                        let Ok(Some(job)) = job else { continue };
                        let reserve = parsed.manifest.budget.score_reserve_usd;
                        let affordable = meter
                            .and_then(Meter::remaining_usd)
                            .is_none_or(|remaining| remaining > reserve);
                        let result = if affordable {
                            score_job(parsed, judges, ctx, checkout, job).map(Some)
                        } else {
                            Ok(None)
                        };
                        *slots[i].lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
                    }
                });
            }
        });
        slots
            .into_iter()
            .map(|slot| slot.into_inner().unwrap_or_else(PoisonError::into_inner))
            .collect()
    }

    /// Trace a scored candidate's measurements, decide it against the
    /// incumbent and record the verdict.
    fn finish(
        &self,
        epoch: u32,
        state: &mut State,
        checkout: &Checkout,
        record: &mut CandidateRecord,
        card: Scorecard,
    ) -> Result<()> {
        let manifest = &self.parsed.manifest;
        emit_measurements(
            &mut state.trace,
            epoch,
            &checkout.label,
            &card,
            &manifest.score.decide.on,
        )?;
        let verdict = score::decide(&manifest.score, &state.card, &card)?;
        state.trace.record(LoopEvent::Scored {
            epoch: epoch.into(),
            candidate: checkout.label.clone(),
            decision: verdict.decision,
            estimate: verdict.estimate,
            lower: verdict.lower,
            upper: verdict.upper,
            units: verdict.units,
        })?;
        record.status = CandidateStatus::Scored;
        record.scorecard = Some(card);
        record.verdict = Some(verdict);
        Ok(())
    }
}

/// What scoring a gated candidate needs.
struct ScoreJob {
    /// The candidate's step folder, where scorer runs keep their output.
    steps: PathBuf,
    /// The candidate's diff against the incumbent, for judges.
    diff: String,
}

/// Score a prepared candidate with the loop's scorers and judges.
fn score_job(
    parsed: &Loop,
    judges: &Judges,
    ctx: &StepContext,
    checkout: &Checkout,
    job: &ScoreJob,
) -> Result<Scorecard> {
    score::score_checkout(
        ctx,
        &parsed.manifest.score,
        checkout,
        &job.steps,
        judges,
        &parsed.rubrics,
        Some(&job.diff),
    )
}

/// A candidate before anything has been done to it.
fn new_record(epoch: u32, seat: &str, checkout: &Checkout) -> CandidateRecord {
    CandidateRecord {
        id: checkout.label.clone(),
        epoch,
        seat: seat.to_owned(),
        checkout: checkout.path.clone(),
        changed: Vec::new(),
        status: CandidateStatus::Empty,
        gates: Vec::new(),
        scorecard: None,
        verdict: None,
        error: None,
    }
}

/// `executed` per seed of the deciding scorer, then `measured`.
fn emit_measurements(
    trace: &mut JsonlTrace,
    epoch: u32,
    label: &str,
    card: &Scorecard,
    on: &str,
) -> Result<()> {
    let runs = card.runs.get(on).map(Vec::as_slice).unwrap_or_default();
    for run in runs {
        trace.record(LoopEvent::Executed {
            epoch: epoch.into(),
            candidate: label.to_owned(),
            seed: run.seed,
            metric: run.result.as_ref().map(|r| r.score),
            ok: run.ok,
            duration_ms: run.duration_ms,
        })?;
    }
    trace.record(LoopEvent::Measured {
        epoch: epoch.into(),
        candidate: label.to_owned(),
        mean: card.headline,
        failures: u32::try_from(runs.iter().filter(|r| !r.ok).count()).unwrap_or(u32::MAX),
        seeds: u32::try_from(runs.len()).unwrap_or(u32::MAX),
    })?;
    Ok(())
}

/// Harvest the notes of this epoch's candidates, `ledger.candidates[first..]`,
/// once promotion has settled their outcomes.
fn harvest(
    spec: &crate::manifest::Carry,
    epoch: u32,
    ledger: &Ledger,
    first: usize,
    dir: &Path,
) -> Result<EpochNotes> {
    let outcomes = history(ledger);
    let sources: Vec<Source<'_>> = ledger.candidates[first..]
        .iter()
        .zip(&outcomes[first..])
        .map(|(c, h)| Source {
            seat: &c.seat,
            candidate: &c.id,
            outcome: &h.outcome,
            checkout: &c.checkout,
            changed: &c.changed,
        })
        .collect();
    let promoted = ledger.epochs.last().and_then(|e| e.promoted.as_deref());
    carry::harvest(spec, epoch, promoted, &sources, dir)
}

/// Earlier decisions, for the brief.
fn history(ledger: &Ledger) -> Vec<HistoryEntry> {
    ledger
        .candidates
        .iter()
        .map(|c| {
            let (outcome, reason) = match (&c.status, &c.verdict) {
                (CandidateStatus::Promoted, Some(v)) => ("promoted", v.reason.clone()),
                (CandidateStatus::Scored, Some(v)) => (
                    match v.decision {
                        ScoreDecision::Accept => "accepted but not best",
                        ScoreDecision::Reject => "rejected",
                        ScoreDecision::Inconclusive => "inconclusive",
                    },
                    format!("{}; estimate {:+.4}", v.reason, v.estimate),
                ),
                (CandidateStatus::Empty, _) => ("empty", "no change".to_owned()),
                (CandidateStatus::Gated, _) => ("gated", c.error.clone().unwrap_or_default()),
                (CandidateStatus::Unaffordable, _) => ("unscored", "budget reserve".to_owned()),
                _ => ("error", c.error.clone().unwrap_or_default()),
            };
            HistoryEntry {
                epoch: c.epoch,
                candidate: c.id.clone(),
                outcome: outcome.to_owned(),
                reason,
            }
        })
        .collect()
}

#[cfg(test)]
mod test;
