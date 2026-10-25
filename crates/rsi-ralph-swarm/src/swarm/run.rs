//! The swarm's async body: runtime, coordinator, seats, brief, drain.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use openhuman_embed::{Access, GroupMode, Provider, Runtime, RuntimeConfig, ToolGroups, Workspace};
use tinyhivemind_hives::{
    Coordinator, Destination, HiveInfo, RunReport, SendMessage, SqliteStorage,
};
use tinyhivemind_lang::LoweredHive;
use tinyhivemind_openhuman::OpenHumanHost;

use super::{RUNTIME_SEAT, SeatUsage, SwarmOutcome, SwarmRequest, seat};
use crate::event::{Redactor, SeatInfo, StopReason, SwarmEvent};
use crate::hooks::SwarmHooks;
use crate::mirror::Mirror;
use crate::package::Hive;
use crate::programs;
use crate::relay::{MAX_RELAYS, relay_message, stall};
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
    let hooks = Arc::new(SwarmHooks::new(trace.clone()).remembered(request.memory.is_some()));
    let mut host = OpenHumanHost::new(runtime.runtime_id().into(), coordinator.clone())?
        .with_turn_timeout(request.turn)?
        .with_hooks(hooks.clone())?;
    if let Some(belt) = programs::belt(&hive, run_dir, |plan| {
        seat::action_dir(plan, &request).to_path_buf()
    }) {
        host = host.with_tools(belt)?;
    }

    let seats = register_seats(&hive, &request, &runtime, &host).await?;
    open_desks(&coordinator, lowered).await?;
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
    let (stop, report) = drain(&coordinator, &trace, &request.lead, request.wall).await?;
    trace.emit(SwarmEvent::SwarmFinished {
        stop,
        completed: report.completed,
        failed: report.failed,
        parked: report.parked,
    });
    trace.finish()?;

    let seats = usage(&hive, hooks.usage());
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

/// Builds and registers one agent per seat, in manifest order.
async fn register_seats(
    hive: &Hive,
    request: &SwarmRequest,
    runtime: &Runtime,
    host: &OpenHumanHost,
) -> Result<Vec<SeatInfo>> {
    let mut seats = Vec::new();
    for plan in &hive.seats {
        let (spec, info) = seat::spec(plan, request, &hive.lowered.context);
        let agent = runtime.agent(spec).map_err(|source| Error::Agent {
            seat: plan.seat.id.clone(),
            source: Box::new(source),
        })?;
        host.register_agent(agent).await?;
        seats.push(info);
    }
    Ok(seats)
}

/// Creates every desk of the package with its members.
async fn open_desks(coordinator: &Coordinator, lowered: &LoweredHive) -> Result<()> {
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
    Ok(())
}

/// Runs the coordinator until idle or `wall`, mirroring its transcript.
///
/// Each time it goes idle with a seat's work hanging ([`stall`]: an unread
/// reply to `lead`, or a message whose turn ended unanswered), the host hands
/// the message back to that seat and the run goes on, at most [`MAX_RELAYS`]
/// times.
async fn drain(
    coordinator: &Coordinator,
    trace: &Arc<Trace>,
    lead: &str,
    wall: Duration,
) -> Result<(StopReason, RunReport)> {
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
    let deadline = tokio::time::Instant::now() + wall;
    let mut total = RunReport::default();
    let mut relays = 0;
    let stop = loop {
        let Ok(report) = tokio::time::timeout_at(deadline, coordinator.run_until_idle()).await
        else {
            break StopReason::WallClock;
        };
        let report = report?;
        total.completed += report.completed;
        total.failed += report.failed;
        total.parked += report.parked;
        if relays == MAX_RELAYS {
            break StopReason::Idle;
        }
        let transcript = coordinator.read_transcript(None)?;
        let Some(stalled) = stall(&transcript, lead) else {
            break StopReason::Idle;
        };
        coordinator.send_as_host(relay_message(&stalled)).await?;
        relays += 1;
    };
    watcher.abort();
    // The abort's cancellation error is the expected result.
    let _ = watcher.await;
    mirror.flush(coordinator, trace)?;
    Ok((stop, total))
}

/// Every seat's tally, with a zero entry for seats that never ran.
fn usage(hive: &Hive, mut tally: BTreeMap<String, SeatUsage>) -> BTreeMap<String, SeatUsage> {
    hive.seats
        .iter()
        .map(|plan| {
            let id = plan.seat.id.clone();
            let seat = tally.remove(&id).unwrap_or_default();
            (id, seat)
        })
        .collect()
}

/// The brief, followed by each candidate's checkout.
pub(super) fn kickoff(request: &SwarmRequest) -> String {
    let mut body = request.brief.clone();
    for (seat, path) in &request.candidates {
        // Writing to a `String` cannot fail.
        let _ = write!(
            body,
            "\n\nCandidate checkout for {seat}: {}",
            path.display()
        );
    }
    body
}

/// The base config every seat's agent is built from: nothing that reaches
/// out or spawns on its own. The memory lifecycle (a recall before and a log
/// after every turn) runs only when seats carry memory (`memory`).
pub(super) fn runtime_config(memory: bool) -> RuntimeConfig {
    let mut config = RuntimeConfig::default();
    config.agent.tool_dispatcher = "native".into();
    config.local_ai.runtime_enabled = false;
    config.memory.recall.enabled = memory;
    config.memory.conversations.enabled = memory;
    config
}

/// The shared `OpenHuman` runtime: read-only by default, with the coding
/// pack's schemas advertised (seats are coding agents), its own model calls
/// tagged [`RUNTIME_SEAT`], and the seats' memory engine when they carry one.
async fn runtime(request: &SwarmRequest, workspace: &Path) -> Result<Runtime> {
    let mut builder = Runtime::builder()
        .config(runtime_config(request.memory.is_some()))
        .workspace(Workspace::dir(workspace))
        .provider(
            Provider::openai_compatible(
                (request.model_base_url)(RUNTIME_SEAT),
                request.model_api_key.clone(),
            )
            .model(request.default_model.clone()),
        )
        .access(Access::readonly())
        .tool_groups(ToolGroups::packed().with("coding", GroupMode::Advertised));
    if let Some(memory) = &request.memory {
        builder = builder.memory_engine(memory.engine());
    }
    // The build future is large; keep it off the caller's stack frame.
    Box::pin(builder.build())
        .await
        .map_err(|error| Error::Runtime(Box::new(error)))
}
