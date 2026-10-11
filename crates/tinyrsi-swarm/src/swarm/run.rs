//! The swarm's async body: runtime, coordinator, seats, brief, drain.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use openhuman_embed::{Access, GroupMode, Provider, Runtime, RuntimeConfig, ToolGroups, Workspace};
use tinyhivemind_hives::{
    Coordinator, Destination, HiveInfo, RunReport, SendMessage, SqliteStorage,
};
use tinyhivemind_openhuman::OpenHumanHost;

use super::{RUNTIME_SEAT, SeatUsage, SwarmOutcome, SwarmRequest, seat};
use crate::event::{Redactor, StopReason, SwarmEvent};
use crate::hooks::SwarmHooks;
use crate::mirror::Mirror;
use crate::package::Hive;
use crate::trace::Trace;
use crate::{Error, Result};

/// The coordinator's durable state, under the run directory.
pub const HIVE_DB: &str = "hive.db";
/// The swarm's event log, under the run directory.
pub const EVENTS_FILE: &str = "events.jsonl";
/// The agents' `OpenHuman` workspace (sessions, transcripts), under the run
/// directory.
pub const RUNTIME_DIR: &str = "runtime";
/// The message id of the brief.
const BRIEF_ID: &str = "brief";
/// How often the transcript is mirrored while seats work.
const MIRROR_INTERVAL: Duration = Duration::from_millis(500);

/// Runs one epoch to idle or the wall clock.
pub(super) async fn run(
    hive: Hive,
    request: SwarmRequest,
    events: Sender<SwarmEvent>,
) -> Result<SwarmOutcome> {
    let run_dir = &request.run_dir;
    std::fs::create_dir_all(run_dir).map_err(|source| Error::RunDir {
        path: run_dir.clone(),
        source,
    })?;
    let mut secrets: Vec<String> = request.tools.iter().map(|key| key.secret.clone()).collect();
    secrets.push(request.model_api_key.clone());
    let trace = Arc::new(Trace::create(
        &run_dir.join(EVENTS_FILE),
        Redactor::new(secrets),
        events,
    )?);
    let runtime = Box::pin(runtime(&request, &run_dir.join(RUNTIME_DIR))).await?;
    let lowered = &hive.lowered;
    let coordinator = Coordinator::new(
        runtime.runtime_id().into(),
        Arc::new(SqliteStorage::open(run_dir.join(HIVE_DB))?),
        tinyhivemind_openhuman::language::coordinator_options(&lowered.coordinator),
    )
    .await?;
    let hooks = Arc::new(SwarmHooks::new(trace.clone()));
    let host = OpenHumanHost::new(runtime.runtime_id().into(), coordinator.clone())?
        .with_turn_timeout(request.turn)?
        .with_hooks(hooks.clone())?;

    let mut seats = Vec::new();
    for plan in &hive.seats {
        let (spec, info) = seat::spec(plan, &request, &lowered.context);
        let agent = runtime.agent(spec).map_err(|source| Error::Agent {
            seat: plan.seat.id.clone(),
            source: Box::new(source),
        })?;
        host.register_agent(agent).await?;
        seats.push(info);
    }
    for desk in &lowered.desks {
        coordinator
            .create_hive(HiveInfo {
                hive_id: desk.id.clone(),
                name: desk.name.clone(),
                description: desk.description.clone(),
                members: Vec::new(),
            })
            .await?;
        for member in &desk.members {
            coordinator.join_hive(&desk.id, member).await?;
        }
    }
    trace.emit(SwarmEvent::SwarmStarted {
        hive: lowered.id.clone(),
        lead: request.lead.clone(),
        desks: lowered.desks.iter().map(|desk| desk.id.clone()).collect(),
        seats,
    });

    coordinator
        .send_as_host(SendMessage {
            message_id: BRIEF_ID.into(),
            sender: String::new(),
            // A direct message, not a desk episode: the epoch then runs as a
            // conversation of seat-to-seat messages, and the coordinator goes
            // idle when the lead stops sending. An episode would re-nudge the
            // lead every round while it waits for an implementer.
            destination: Destination::Agent(request.lead.clone()),
            body: kickoff(&request),
            thread: None,
            only_for: Vec::new(),
            starters: Vec::new(),
        })
        .await?;

    let mirror = Arc::new(Mirror::default());
    let watcher = tokio::spawn({
        let (mirror, coordinator, trace) = (mirror.clone(), coordinator.clone(), trace.clone());
        async move {
            loop {
                // A failed read is retried next tick and by the final flush.
                let _ = mirror.flush(&coordinator, &trace);
                tokio::time::sleep(MIRROR_INTERVAL).await;
            }
        }
    });
    let drained = tokio::time::timeout(request.wall, coordinator.run_until_idle()).await;
    watcher.abort();
    // The abort's cancellation error is the expected result.
    let _ = watcher.await;
    mirror.flush(&coordinator, &trace)?;
    let (stop, report) = match drained {
        Ok(report) => (StopReason::Idle, report?),
        Err(_) => (StopReason::WallClock, RunReport::default()),
    };
    trace.emit(SwarmEvent::SwarmFinished {
        stop,
        completed: report.completed,
        failed: report.failed,
        parked: report.parked,
    });
    trace.finish()?;

    let mut usage = hooks.usage();
    let seats = hive
        .seats
        .iter()
        .map(|plan| {
            let id = plan.seat.id.clone();
            let seat = usage.remove(&id).unwrap_or_else(SeatUsage::default);
            (id, seat)
        })
        .collect();
    // Release every handle on the runtime before the next swarm builds one.
    drop(host);
    drop(coordinator);
    drop(runtime);
    Ok(SwarmOutcome {
        stop,
        completed: report.completed,
        failed: report.failed,
        parked: report.parked,
        seats,
    })
}

/// The brief, followed by each candidate's checkout.
pub(super) fn kickoff(request: &SwarmRequest) -> String {
    let mut body = request.brief.clone();
    for (seat, path) in &request.candidates {
        body.push_str(&format!(
            "\n\nCandidate checkout for {seat}: {}",
            path.display()
        ));
    }
    body
}

/// The base config every seat's agent is built from: nothing that reaches
/// out or spawns on its own. Seats carry no memory engine, so the memory
/// lifecycle (a recall before and a log after every turn) is off.
pub(super) fn runtime_config() -> RuntimeConfig {
    let mut config = RuntimeConfig::default();
    config.agent.tool_dispatcher = "native".into();
    config.local_ai.runtime_enabled = false;
    config.memory.recall.enabled = false;
    config.memory.conversations.enabled = false;
    config
}

/// The shared `OpenHuman` runtime: read-only by default, with the coding
/// pack's schemas advertised (seats are coding agents), and its own model
/// calls tagged [`RUNTIME_SEAT`].
async fn runtime(request: &SwarmRequest, workspace: &Path) -> Result<Runtime> {
    Runtime::builder()
        .config(runtime_config())
        .workspace(Workspace::dir(workspace))
        .provider(
            Provider::openai_compatible(
                (request.model_base_url)(RUNTIME_SEAT),
                request.model_api_key.clone(),
            )
            .model(request.default_model.clone()),
        )
        .access(Access::readonly())
        .tool_groups(ToolGroups::packed().with("coding", GroupMode::Advertised))
        .build()
        .await
        .map_err(|error| Error::Runtime(Box::new(error)))
}
