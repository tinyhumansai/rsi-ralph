//! The swarm's async body: runtime, coordinator, seats, brief, drain.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use openhuman_embed::{
    Access, GroupMode, Provider, Runtime, RuntimeBuilder, RuntimeConfig, RuntimeError, ToolGroups,
    Workspace,
};
use tinyhivemind_hives::{
    Coordinator, Destination, HiveInfo, RunReport, SendMessage, SqliteStorage,
};
use tinyhivemind_lang::{LoweredHive, SessionSettings};
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
/// How long a swarm waits for the previous swarm's runtime to be released.
/// A seat whose turn the wall clock cut off keeps `OpenHuman`'s process-wide
/// runtime slot until that turn has wound down, typically within a second.
const RELEASE_WAIT: Duration = Duration::from_secs(30);
/// How often the slot is retried while it is still held.
const RELEASE_POLL: Duration = Duration::from_millis(50);

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
    let runtime = Box::pin(runtime(
        &request,
        &hive.lowered.integrations.session,
        &run_dir.join(RUNTIME_DIR),
    ))
    .await?;
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
/// after every turn) runs only when seats carry memory (`memory`). Each
/// setting the hive's `session` gives replaces `OpenHuman`'s default.
pub(super) fn runtime_config(memory: bool, session: &SessionSettings) -> RuntimeConfig {
    let mut config = RuntimeConfig::default();
    config.agent.tool_dispatcher = "native".into();
    config.local_ai.runtime_enabled = false;
    config.memory.recall.enabled = memory;
    config.memory.conversations.enabled = memory;
    apply_session(&mut config, session);
    config
}

/// Applies the hive's `session` settings the way tinyhivemind's own deploy
/// does: every field set replaces the matching `agent` setting.
fn apply_session(config: &mut RuntimeConfig, session: &SessionSettings) {
    let agent = &mut config.agent;
    if let Some(value) = session.compact_context {
        agent.compact_context = value;
    }
    if let Some(value) = session.max_history_messages {
        agent.max_history_messages = value;
    }
    if let Some(value) = session.tool_result_budget_bytes {
        agent.tool_result_budget_bytes = value;
    }
    if let Some(value) = session.max_tool_iterations {
        agent.max_tool_iterations = value;
    }
    if let Some(value) = session.parallel_tools {
        agent.parallel_tools = value;
    }
    if let Some(value) = session.agents_md {
        agent.agents_md_enabled = value;
    }
}

/// The shared `OpenHuman` runtime: read-only by default, with the coding
/// pack's schemas advertised (seats are coding agents), its own model calls
/// tagged [`RUNTIME_SEAT`], the hive's `session` settings, and the seats'
/// memory engine when they carry one.
async fn runtime(
    request: &SwarmRequest,
    session: &SessionSettings,
    workspace: &Path,
) -> Result<Runtime> {
    // The build future is large; keep it off the caller's stack frame.
    claim(RELEASE_WAIT, RELEASE_POLL, || {
        Box::pin(builder(request, session, workspace).build())
    })
    .await
    .map_err(|error| Error::Runtime(Box::new(error)))
}

/// The runtime's builder for `request` and the hive's `session` settings,
/// with its workspace at `workspace`.
fn builder(request: &SwarmRequest, session: &SessionSettings, workspace: &Path) -> RuntimeBuilder {
    let builder = Runtime::builder()
        .config(runtime_config(request.memory.is_some(), session))
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
    match &request.memory {
        Some(memory) => builder.memory_engine(memory.engine()),
        None => builder,
    }
}

/// Runs `build` until it stops answering [`RuntimeError::AlreadyRunning`],
/// retrying every `poll` for up to `wait`.
///
/// Swarms in one process take turns (`run_swarm` holds its lock), so a slot
/// that is still claimed belongs to the previous swarm's teardown, which
/// releases it on its own. Any other result, or the last refusal once `wait`
/// is spent, is returned as is.
pub(super) async fn claim<T, F, Fut>(
    wait: Duration,
    poll: Duration,
    mut build: F,
) -> std::result::Result<T, RuntimeError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::result::Result<T, RuntimeError>>,
{
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        match build().await {
            Err(RuntimeError::AlreadyRunning) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(poll).await;
            }
            settled => return settled,
        }
    }
}
