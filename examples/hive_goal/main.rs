//! Orchestrate a hive of agents to achieve a goal.
//!
//! The subject is a tinyhivemind hive (a `Package`): a seat on a desk, a
//! goal ("answer every assigned arithmetic task exactly") and a shared
//! context document every agent reads. That document starts out unhelpful
//! ("guess, and reply UNKNOWN"), so the hive fails. The hive path of this
//! library improves it, one epoch at a time:
//!
//! 1. **Observe.** The host runs the archive's head in a live SQLite
//!    coordinator (`DeploymentStore` + `CoordinatorHost`), sends it a public
//!    probe task, checks the reply and builds a public failure `Dossier`.
//! 2. **Propose.** `prepare_retreat_epoch` asks a proposal model, through the
//!    epoch driver's `GenerationMode::Direct` arm, for typed `Proposal`s
//!    (tinyhivemind patches) grounded in that dossier.
//! 3. **Evaluate.** `evaluate_retreat_epoch` runs each proposal through
//!    `run_proposal_h`: class, guard and grounding checks first, then paired
//!    screen, selection and confirmation runs of incumbent against candidate
//!    on host-private tasks (golden-answer and JSON-schema checkers), scored
//!    as normalized H, then the approval gate.
//! 4. **Deploy.** The first accepted hive is activated in a fresh coordinator
//!    and sent the probe; the goal is reached when it passes.
//!
//! A failed probe leaves its episode waiting on the incumbent, so the host
//! cannot swap packages in the observation coordinator; the accepted hive is
//! deployed in a new one, as a production host would at a safe boundary.
//!
//! Offline (the default) everything is local and deterministic: no network,
//! no credentials, a few seconds. The agents are a `ContextFollower` model
//! that does what its shared context says, so only the right context makes
//! the private checkers pass. The proposer replays a script: in epoch 1 a
//! seat-prompt edit (rejected `WRONG_CLASS`: only context edits are allowed),
//! a context item sending the agents to a calculator tool the hive lacks
//! (rejected `HARMFUL`: the incumbent completes its private runs, the
//! candidate crashes on every one), a reworded copy of that item (stopped
//! `COOLDOWN` with no evaluation: same patch shape, fresh strike) and a
//! context edit asking for brevity (rejected `INCONCLUSIVE`: it changes
//! nothing measurable); in epoch 2 the real repair (`ACCEPTED`). Normalized H
//! moves from -0.10 (nothing solved, less the false-claim penalty for every
//! failed check) to about 1.00. The run exits non-zero if no proposal is
//! accepted or the deployed hive still fails, and offline also if the
//! crashing edit is not penalized.
//!
//! With `TINYRSI_LIVE_OPENROUTER=1` (plus `OPENROUTER_API_KEY`;
//! `OPENROUTER_MODEL` picks the model) one real model plays both the agents
//! and the proposer, so what gets proposed and accepted is up to it.
//!
//! The loop trace lands in `target/rsi-examples/hive_goal/<run>/events.jsonl`
//! next to the archive, deployment checkpoints and epoch journal.
mod hive;
mod models;

use std::{collections::BTreeMap, error::Error, path::Path, sync::Arc};
use tinyhivemind_hives::{Destination, SendMessage};
use tinyrsi_examples::{live, live_or, run_location};
use tinyrsi_host::{
    Archive, Budget, ControlledRunnerFactory, CoordinatorEvaluator, CoordinatorHost,
    DeploymentStore, Dossier, DurableRetreatMemory, Epoch, EpochJournal, EvalPolicy,
    GenerationBudget, GenerationMode, HEpoch, JsonlTrace, LoopEvent, MemoryHealth, MemoryLimits,
    MetricDirection, ModelPort, MonotonicClock, NodeStatus, PatchClass, ReasonCode,
    RetreatEpochInput, RunGoal, SnapshotMemoryStore, StopReason, TraceSink, VerifierOutcome,
    evaluate_retreat_epoch, prepare_retreat_epoch, record_epoch,
};

/// Epochs before the run gives up; offline needs two.
const MAX_EPOCHS: u64 = 3;
/// Hive identity in generation requests and retreat memory.
const HIVE_ID: &str = "hive-goal";
/// The normalized H an accepted hive should reach (1.0 is every task solved).
const TARGET_H: f64 = 0.9;
/// Public instructions the proposer is given every epoch.
const ORCHESTRATOR: &str = "Repair the hive's shared context so its agents solve their \
assigned tasks. Edit only context items, ground every proposal in the public dossier and \
predict normalized_h.";

type Outcome<T> = Result<T, Box<dyn Error>>;

/// An accepted proposal: its title and the confirmed incumbent and candidate
/// normalized H.
struct Accepted {
    title: String,
    from: f64,
    to: f64,
}

fn main() -> Outcome<()> {
    let run = run_location("hive_goal")?;
    std::fs::create_dir_all(&run.dir)?;
    let (agents, proposer) = models()?;

    let mut archive = Archive::open(run.dir.join("archive.json"))?;
    let baseline = hive::found(&mut archive)?;
    let memory = SnapshotMemoryStore::open(run.dir.join("hive-memory.json"), vec![], 1_000_000)?;
    memory.seed(&hive::package()?, &BTreeMap::new(), BTreeMap::new())?;
    let trace_path = run.dir.join("events.jsonl");
    let mut hive_loop = HiveLoop {
        // Built before the host's runtime exists: the evaluator owns its own.
        evaluator: CoordinatorEvaluator::new(
            hive::suites()?,
            agents.clone(),
            memory.clone(),
            256,
            60_000,
        )?,
        observed: LiveHive::open(&run.dir.join("observation"), agents.clone(), &memory)?,
        proposer,
        discovery: Arc::new(DurableRetreatMemory::open(
            run.dir.join("retreat-memory.json"),
            HIVE_ID,
            MemoryLimits::default(),
            vec![],
        )?),
        journal: EpochJournal::open(run.dir.join("epochs.json"))?,
        trace: JsonlTrace::create(&trace_path, MonotonicClock::new())?,
        runtime: tokio::runtime::Runtime::new()?,
        archive,
        rejected: 0,
        reasons: vec![],
    };
    hive_loop.trace.record(LoopEvent::RunStarted {
        run_id: run.id.clone(),
        subject: "hive: arithmetic solver".into(),
        goal: RunGoal {
            metric: "normalized_h".into(),
            direction: MetricDirection::Maximize,
            target: Some(TARGET_H),
        },
    })?;

    let mut accepted = None;
    for label in 1..=MAX_EPOCHS {
        accepted = hive_loop.epoch(label)?;
        if accepted.is_some() {
            break;
        }
    }

    let penalized = [ReasonCode::Harmful, ReasonCode::Cooldown]
        .iter()
        .all(|reason| hive_loop.reasons.contains(reason));
    if !live() && !penalized {
        return Err("the crashing edit was not rejected HARMFUL and cooled".into());
    }
    let HiveLoop {
        archive,
        mut trace,
        runtime,
        ..
    } = hive_loop;
    let incumbent = archive.head().ok_or("archive has no head")?.id.clone();
    let reached = if accepted.is_some() {
        let mut deployed = LiveHive::open(&run.dir.join("deployment"), agents, &memory)?;
        let (_, solved) = runtime.block_on(deployed.probe(&archive, "probe-deployed"))?;
        println!(
            "deployed {}: probe {}",
            short(&incumbent),
            if solved { "passed" } else { "failed" }
        );
        solved
    } else {
        false
    };
    trace.record(LoopEvent::RunFinished {
        reason: if reached {
            StopReason::GoalReached
        } else {
            StopReason::MaxEpochs
        },
        incumbent: incumbent.clone(),
        incumbent_metric: accepted.as_ref().map(|a| a.to),
    })?;

    println!();
    println!("baseline hive   {}", short(&baseline));
    println!("improved hive   {}", short(&incumbent));
    println!("shared context  {:?}", context(&archive, &incumbent)?);
    println!("trace           {}", trace_path.display());
    let accepted = accepted.ok_or("no proposal was accepted")?;
    println!(
        "accepted        {:?}: normalized_h {:.2} -> {:.2}",
        accepted.title, accepted.from, accepted.to
    );
    if !reached || accepted.to < TARGET_H {
        return Err("the improved hive did not reach its goal".into());
    }
    println!("goal reached: the deployed hive solves its tasks");
    Ok(())
}

/// Everything one improvement epoch reads and writes.
struct HiveLoop {
    archive: Archive,
    evaluator: CoordinatorEvaluator,
    observed: LiveHive,
    proposer: Arc<dyn ModelPort>,
    discovery: Arc<DurableRetreatMemory>,
    journal: EpochJournal,
    trace: JsonlTrace,
    runtime: tokio::runtime::Runtime,
    /// Proposals rejected so far, across epochs.
    rejected: u32,
    /// Every reason a proposal stopped with, across epochs.
    reasons: Vec<ReasonCode>,
}

impl HiveLoop {
    /// Observes the incumbent, generates proposals and evaluates them in
    /// rank order, recording the epoch on the trace as `label`. Returns the
    /// accepted proposal, if any.
    fn epoch(&mut self, label: u64) -> Outcome<Option<Accepted>> {
        let incumbent = self.archive.head().ok_or("archive has no head")?.id.clone();
        self.trace.record(LoopEvent::EpochStarted {
            epoch: label,
            incumbent: incumbent.clone(),
        })?;
        let (dossier, solved) = self.runtime.block_on(
            self.observed
                .probe(&self.archive, &format!("probe-{label}")),
        )?;
        self.trace.record(LoopEvent::Observed {
            epoch: label,
            incumbent: incumbent.clone(),
            incumbent_metric: None,
            history: u32::try_from(label - 1)?,
            rejected: self.rejected,
        })?;
        println!(
            "epoch {label}: probe {} ({} transcript refs, failed nodes {:?})",
            if solved { "passed" } else { "failed" },
            dossier.transcript_sequences.len(),
            dossier.failed_nodes,
        );
        if solved {
            return Err("the founding hive already solves the probe; nothing to improve".into());
        }
        let epoch = h_epoch(&dossier, &self.evaluator);
        let prepared = self.runtime.block_on(prepare_retreat_epoch(
            &mut self.journal,
            self.proposer.clone(),
            self.discovery.clone(),
            RetreatEpochInput {
                generation_seed: 7000 + label,
                orchestrator_context: ORCHESTRATOR,
                hive_id: HIVE_ID,
                archive: &self.archive,
                checkpoint: self.observed.store.checkpoint(),
                dossier: &dossier,
                epoch: &epoch,
                mode: GenerationMode::Direct,
                budget: &generation(),
            },
        ))?;
        let outcome = evaluate_retreat_epoch(
            &mut self.journal,
            &mut self.archive,
            &epoch,
            &mut self.evaluator,
            &mut hive::ContextOnlyApproval,
            &self.discovery,
        )?;
        record_epoch(&mut self.trace, label, &self.archive, &prepared, &outcome)?;
        let mut accepted = None;
        for candidate in &outcome.candidates {
            let proposal = &prepared.proposals[candidate.proposal_index];
            println!(
                "  p{label}-{} {:<12} {}",
                candidate.proposal_index,
                code(candidate.reason)?,
                proposal.title
            );
            self.reasons.push(candidate.reason);
            if let Some(failure) = self
                .archive
                .candidate_failures()
                .into_iter()
                .find(|f| Some(&f.candidate_id) == candidate.candidate_id.as_ref())
            {
                println!(
                    "      candidate failed {}/{} private runs the incumbent completed ({:?})",
                    failure.failed_runs, failure.runs, failure.kind
                );
            }
            if candidate.reason == ReasonCode::Cooldown {
                println!("      same patch shape as a struck edit: not evaluated");
            }
            if candidate.reason == ReasonCode::Accepted {
                let id = candidate
                    .candidate_id
                    .as_deref()
                    .ok_or("accepted without id")?;
                let (from, to) = confirmed(&self.archive, id)?;
                println!("      normalized_h {from:.2} -> {to:.2} (confirmation draw)");
                accepted = Some(Accepted {
                    title: proposal.title.clone(),
                    from,
                    to,
                });
            } else {
                self.rejected += 1;
            }
        }
        Ok(accepted)
    }
}

/// The agents' and the proposer's model ports: offline fakes by default, one
/// real model for both in live mode.
fn models() -> Outcome<(Arc<dyn ModelPort>, Arc<dyn ModelPort>)> {
    if live() {
        let model = live_or(Arc::new(models::ContextFollower))?;
        return Ok((model.clone(), model));
    }
    Ok((
        Arc::new(models::ContextFollower),
        Arc::new(models::ScriptedProposer::default()),
    ))
}

/// One running deployment of the hive: a SQLite coordinator under `dir`
/// running the archive's head, answering the public probe through a
/// host-checked runner.
struct LiveHive {
    store: DeploymentStore,
    host: CoordinatorHost,
    factory: Arc<ControlledRunnerFactory>,
    /// Checker outcomes and usage records already folded into a dossier.
    seen: (usize, usize),
}

impl LiveHive {
    fn open(dir: &Path, agents: Arc<dyn ModelPort>, memory: &SnapshotMemoryStore) -> Outcome<Self> {
        let factory = Arc::new(ControlledRunnerFactory::new(
            agents,
            memory.clone(),
            hive::probe()?,
            Budget { max_tokens: 16_384 },
            256,
            60_000,
        )?);
        Ok(Self {
            store: DeploymentStore::open(dir.join("deployment.json"))?,
            host: CoordinatorHost::sqlite(
                HIVE_ID.into(),
                dir.join("hive.sqlite"),
                factory.clone(),
                Arc::new(memory.clone()),
            )?,
            factory,
            seen: (0, 0),
        })
    }

    /// Runs the archive's head (a no-op when it already runs), sends it the
    /// probe, and checkpoints one observed epoch. Returns its public dossier
    /// and whether every probe check passed.
    async fn probe(&mut self, archive: &Archive, message_id: &str) -> Outcome<(Dossier, bool)> {
        self.store.activate(archive, &mut self.host).await?;
        let coordinator = self.host.coordinator().ok_or("hive is not running")?;
        coordinator
            .send_as_host(SendMessage {
                message_id: message_id.into(),
                sender: "host".into(),
                thread: None,
                only_for: vec![],
                starters: vec![],
                destination: Destination::Hive(hive::DESK.into()),
                body: "Public probe: answer the assigned arithmetic task.".into(),
            })
            .await?;
        coordinator.run_until_idle().await?;
        let observation = self.store.observe(coordinator)?;
        // A failed check parks the seat until the host settles it; this host
        // has read the outcome, so it frees the seat for the next probe.
        coordinator.release(hive::SEAT).await?;
        let evidence = self.factory.evidence()?;
        let usage = self.factory.usage()?;
        let new_evidence = &evidence[self.seen.0..];
        let new_usage = &usage[self.seen.1..];
        self.seen = (evidence.len(), usage.len());
        let outcomes: Vec<_> = new_evidence
            .iter()
            .map(|e| VerifierOutcome {
                node_id: e.binding.node_id.clone(),
                status: if e.passed {
                    NodeStatus::Verified
                } else {
                    NodeStatus::Failed
                },
            })
            .collect();
        let solved = !new_evidence.is_empty() && new_evidence.iter().all(|e| e.passed);
        let dossier = Dossier::build(
            self.store.checkpoint().epoch,
            &observation,
            new_usage,
            &outcomes,
            MemoryHealth::default(),
        );
        Ok((dossier, solved))
    }
}

/// The frozen evaluation epoch for one observed dossier: context edits only,
/// the hive's scoring policy and the evaluator's private suite ids.
fn h_epoch(dossier: &Dossier, evaluator: &CoordinatorEvaluator) -> HEpoch {
    let mut nodes = dossier.failed_nodes.clone();
    nodes.extend(dossier.open_nodes.iter().cloned());
    nodes.sort();
    nodes.dedup();
    HEpoch {
        epoch: Epoch {
            id: dossier.epoch,
            class: PatchClass::Context,
            transcript_refs: dossier.transcript_sequences.clone(),
            failure_nodes: nodes,
            budget: Budget { max_tokens: 16_384 },
            penalty: tinyrsi_host::PenaltyPolicy::default(),
            policy: EvalPolicy::default(),
        },
        policy: hive::policy(),
        suite_ids: evaluator.suite_ids(),
    }
}

/// One direct proposer call per epoch returning at most four proposals.
fn generation() -> GenerationBudget {
    GenerationBudget {
        total_tokens: 200_000,
        call_tokens: 65_536,
        output_tokens: 4096,
        timeout_ms: 120_000,
        variations: 1,
        generations: 1,
        items_per_variation: 4,
        select_top_k: 4,
        judge_raters: 1,
        wall_clock_secs: 120,
    }
}

/// The incumbent and candidate normalized H of a candidate's last
/// (confirmation) comparison.
fn confirmed(archive: &Archive, candidate: &str) -> Outcome<(f64, f64)> {
    let evaluation = archive
        .normalized_evaluations()
        .into_iter()
        .rev()
        .find(|e| e.candidate_id == candidate)
        .ok_or("accepted candidate has no evaluation")?;
    Ok((
        evaluation.comparison.incumbent.value,
        evaluation.comparison.candidate.value,
    ))
}

/// The wire form of a reason code, e.g. `WRONG_CLASS`.
fn code(reason: ReasonCode) -> Outcome<String> {
    Ok(serde_json::to_value(reason)?
        .as_str()
        .ok_or("reason code is not a string")?
        .to_owned())
}

/// The shared context text of the archived package `id`.
fn context(archive: &Archive, id: &str) -> Outcome<String> {
    let document = archive
        .package(id)
        .and_then(|p| p.documents.get(hive::CONTEXT))
        .ok_or("package has no shared context")?;
    Ok(document
        .lines()
        .filter(|line| !line.starts_with("<!--"))
        .collect::<Vec<_>>()
        .join(" "))
}

/// A content id shortened for display.
fn short(id: &str) -> &str {
    id.get(..19).unwrap_or(id)
}
