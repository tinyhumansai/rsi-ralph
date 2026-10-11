//! The epoch loop.
//!
//! One run: score the incumbent (or reuse a cached score for the same
//! revision and scorers), then per epoch render the brief, optionally ask the
//! retreat for ideas, give each editing seat its own candidate checkout, run
//! the hive, and for every changed candidate run the surface check, the
//! gates, the scorers and the decision. The best accepted candidate is
//! promoted. The run stops on `goal_reached`, `stalled`, `budget`,
//! `max_epochs` or `error`, and every step is mirrored to `events.jsonl`,
//! `ledger.json` and `run.json` as it happens.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use tinyrsi_score::ScoreDecision;
use tinyrsi_trace::{JsonlTrace, LoopEvent, MonotonicClock, RunGoal, StopReason, TraceSink};

use crate::{
    Error, Result,
    brief::{self, BriefInput, HistoryEntry},
    gate,
    manifest::{DecideTest, Loop},
    score::{self, Judges, Scorecard},
    step::StepContext,
    store::{self, RunDir, RunRecord, RunStatus, Store},
    subject::{Checkout, Subject},
};

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
}

/// Mutable state of one run.
struct State {
    trace: JsonlTrace,
    record: RunRecord,
    ledger: Ledger,
    incumbent: Checkout,
    revision: String,
    card: Scorecard,
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
        };
        store::write_json(&self.run.record_path(), &record)?;
        let result = self.execute(&mut record);
        record.finished_at = Some(store::now());
        record.spend_usd = self.meter.map_or(0.0, Meter::spent_usd);
        match &result {
            Ok(summary) => {
                record.status = RunStatus::Finished;
                record.stop_reason = Some(summary.reason);
            }
            Err(e) => {
                record.status = RunStatus::Failed;
                record.stop_reason = Some(StopReason::Error);
                record.error = Some(store::scrub(
                    &e.to_string(),
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
                    DecideTest::Sign => tinyrsi_score::MetricDirection::Maximize,
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
    /// under the same scorers.
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
            let digest = hasher.finalize();
            digest.iter().take(8).map(|b| format!("{b:02x}")).collect::<String>()
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
            let promoted = self.epoch(epoch, subject, state)?;
            if let Some(ideator) = self.ideator.as_deref_mut() {
                ideator.outcome(epoch, promoted)?;
            }
            stalled = if promoted { 0 } else { stalled + 1 };
            state.trace.record(LoopEvent::EpochFinished {
                epoch: epoch.into(),
                incumbent: state.revision.clone(),
                incumbent_metric: state.card.headline,
            })?;
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

    /// One epoch; returns whether a candidate was promoted.
    fn epoch(&mut self, epoch: u32, subject: &Subject, state: &mut State) -> Result<bool> {
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
        let input = |ideas: &'_ [String]| {
            brief::render(
                &self.parsed.brief,
                &BriefInput {
                    incumbent: &state.card,
                    decide_on: &manifest.score.decide.on,
                    history: &history,
                    ideas,
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
            let checkout =
                subject.candidate(&state.incumbent, &label, &branch, &dir.join("candidates").join(&seat))?;
            candidates.insert(seat, checkout);
        }
        let report = self.proposer.propose(&ProposalRequest {
            epoch,
            brief: &text,
            incumbent: &state.incumbent,
            candidates: &candidates,
            dir: &dir.join("hive"),
        })?;
        state.ledger.epochs.push(EpochRecord {
            epoch,
            incumbent: state.revision.clone(),
            report,
            ideas,
            promoted: None,
        });

        let mut accepted: Vec<usize> = Vec::new();
        for (seat, checkout) in &candidates {
            let index = self.consider(epoch, seat, checkout, subject, state, &dir)?;
            if let Some(i) = index {
                accepted.push(i);
            }
            self.save(state)?;
        }
        let best = accepted.into_iter().reduce(|a, b| {
            let score = |i: usize| state.ledger.candidates[i].scorecard.as_ref().and_then(|c| c.headline);
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
        let card = winner.scorecard.clone().unwrap_or_else(|| state.card.clone());
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

    /// Gate, score and decide one candidate. Returns its ledger index when
    /// it was accepted.
    fn consider(
        &mut self,
        epoch: u32,
        seat: &str,
        checkout: &Checkout,
        subject: &Subject,
        state: &mut State,
        dir: &Path,
    ) -> Result<Option<usize>> {
        let mut record = CandidateRecord {
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
        };
        let outcome = self.judge(epoch, checkout, subject, state, dir, &mut record);
        if let Err(e) = outcome {
            record.status = CandidateStatus::Error;
            record.error = Some(store::scrub(
                &e.to_string(),
                self.secrets.iter().map(String::as_str),
            ));
        }
        let accepted = record.status == CandidateStatus::Scored
            && record
                .verdict
                .as_ref()
                .is_some_and(|v| v.decision == ScoreDecision::Accept);
        state.ledger.candidates.push(record);
        Ok(accepted.then(|| state.ledger.candidates.len() - 1))
    }

    fn judge(
        &mut self,
        epoch: u32,
        checkout: &Checkout,
        subject: &Subject,
        state: &mut State,
        dir: &Path,
        record: &mut CandidateRecord,
    ) -> Result<()> {
        let manifest = &self.parsed.manifest;
        record.changed = subject.changes(&state.incumbent, checkout)?;
        if record.changed.is_empty() {
            return Ok(());
        }
        state.trace.record(LoopEvent::Ideated {
            epoch: epoch.into(),
            proposal_id: checkout.label.clone(),
            parent: state.revision.clone(),
            candidate: Some(checkout.label.clone()),
            rationale: format!("{} changed {} files", record.seat, record.changed.len()),
            hypothesis: record.changed.join(", "),
        })?;
        let steps = dir.join("steps").join(&checkout.label);
        let ctx = self.context(epoch, &state.incumbent);
        record.gates.push(gate::surface(&manifest.subject, &record.changed));
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
                units: tinyrsi_trace::IntervalUnits::Quality,
            })?;
            record.error.clone_from(&failed.reason);
            return Ok(());
        }
        if !self.affordable() {
            record.status = CandidateStatus::Unaffordable;
            return Ok(());
        }
        let diff = subject.diff(&state.incumbent, checkout)?;
        store::write_atomic(&steps.join("change.diff"), diff.as_bytes())?;
        let card = score::score_checkout(
            &ctx,
            &manifest.score,
            checkout,
            &steps,
            &self.judges,
            &self.parsed.rubrics,
            Some(&diff),
        )?;
        emit_measurements(&mut state.trace, epoch, &checkout.label, &card, &manifest.score.decide.on)?;
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
