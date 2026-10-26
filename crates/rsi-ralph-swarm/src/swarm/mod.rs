//! [`run_swarm`]: one epoch of a hive package, run as `OpenHuman` agents.
//!
//! The request is checked on the caller's thread. The swarm then runs on a
//! dedicated thread with a 16 MB stack and its own Tokio runtime (an agent
//! turn's future is deep), streaming events back to the caller's progress
//! callback until it finishes. Dropping that runtime at the end cancels
//! anything a wall-clock stop left in flight, so no seat keeps editing after
//! `run_swarm` returns.

mod run;
mod seat;
mod types;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError, mpsc};

pub use run::{EVENTS_FILE, HIVE_DB, RUNTIME_DIR};
pub use types::{ModelUrl, SeatUsage, SwarmOutcome, SwarmRequest};

use crate::event::SwarmEvent;
use crate::package::Hive;
use crate::{Error, Result, builtin, programs, tools};

/// The name [`SwarmRequest::model_base_url`] is called with for the
/// runtime's own model calls; no seat may use it.
pub const RUNTIME_SEAT: &str = "runtime";

/// Stack size of the swarm thread and of its runtime's workers.
const STACK_BYTES: usize = 16 * 1024 * 1024;

/// `OpenHuman` allows one embedded runtime per process; swarms take turns.
static RUNNING: Mutex<()> = Mutex::new(());

/// Runs one epoch: builds an `OpenHuman` agent per seat, sends `brief` to
/// the lead as a direct message, and runs the coordinator until no seat has
/// work left or [`SwarmRequest::wall`] elapses.
///
/// Each seat's system prompt is its seat prompt, its role charter, its
/// working directory, the package's shared context and the hive rules
/// ([`crate::HIVE_RULES`]). Its tools are its package allow list plus the
/// permanent `hivemind_*` tools, any tool-key tools and its executable tools
/// (declared under `tools` in `hive.json`), minus its deny list
/// and always minus `wait`/`wait_loop`. Editing seats get full access to
/// their candidate checkout; every other seat reads the incumbent.
///
/// `progress` receives every [`SwarmEvent`] on the calling thread, after it
/// has been scrubbed of secrets and appended to `events.jsonl`. Swarms in
/// one process run one at a time. A seat turn the wall clock cut off can hold
/// `OpenHuman`'s one-per-process runtime for a moment after this returns; the
/// next swarm waits up to 30 seconds for it before failing with
/// [`Error::Runtime`].
///
/// # Errors
///
/// Request errors before anything starts: [`Error::Package`],
/// [`Error::UnknownLead`], [`Error::ReservedSeat`],
/// [`Error::MissingCandidate`], [`Error::UnexpectedCandidate`],
/// [`Error::Checkout`], [`Error::UnsupportedTool`],
/// [`Error::UnknownToolSeat`] and [`Error::PackageProgram`]. During the run: [`Error::RunDir`],
/// [`Error::Runtime`], [`Error::Agent`], [`Error::Adapter`] and
/// [`Error::Coordinator`]; and [`Error::Thread`] or [`Error::ThreadPanicked`]
/// when the swarm thread itself fails.
pub fn run_swarm(
    mut request: SwarmRequest,
    progress: &mut dyn FnMut(SwarmEvent),
) -> Result<SwarmOutcome> {
    let hive = check(&mut request)?;
    let _turn = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .name("rsi-ralph-swarm".into())
        .stack_size(STACK_BYTES)
        .spawn(move || drive(hive, request, sender))
        .map_err(Error::Thread)?;
    for event in receiver {
        progress(event);
    }
    worker.join().map_err(|_| Error::ThreadPanicked)?
}

/// Validates `request` against its package and resolves its checkouts to
/// absolute paths.
fn check(request: &mut SwarmRequest) -> Result<Hive> {
    let hive = Hive::open(&request.package)?;
    let seats: BTreeSet<&str> = hive
        .seats
        .iter()
        .map(|plan| plan.seat.id.as_str())
        .collect();
    if seats.contains(RUNTIME_SEAT) {
        return Err(Error::ReservedSeat(RUNTIME_SEAT.into()));
    }
    if !seats.contains(request.lead.as_str()) {
        return Err(Error::UnknownLead(request.lead.clone()));
    }
    for plan in &hive.seats {
        if plan.editing && !request.candidates.contains_key(&plan.seat.id) {
            return Err(Error::MissingCandidate(plan.seat.id.clone()));
        }
    }
    for seat in request.candidates.keys() {
        if !hive
            .seats
            .iter()
            .any(|plan| plan.editing && &plan.seat.id == seat)
        {
            return Err(Error::UnexpectedCandidate(seat.clone()));
        }
    }
    tools::validate(&request.tools, &seats)?;
    programs::validate(&hive)?;
    builtin::validate(&hive)?;
    request.incumbent = directory(&request.incumbent)?;
    for path in request.candidates.values_mut() {
        *path = directory(path)?;
    }
    Ok(hive)
}

/// `path` made absolute, if it is an existing directory.
fn directory(path: &Path) -> Result<PathBuf> {
    let checkout = |source| Error::Checkout {
        path: path.to_path_buf(),
        source,
    };
    let resolved = std::fs::canonicalize(path).map_err(checkout)?;
    if resolved.is_dir() {
        Ok(resolved)
    } else {
        Err(checkout(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "not a directory",
        )))
    }
}

/// The swarm thread: its own runtime, with the run as a spawned task so its
/// futures get the workers' large stacks.
fn drive(
    hive: Hive,
    request: SwarmRequest,
    events: mpsc::Sender<SwarmEvent>,
) -> Result<SwarmOutcome> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(STACK_BYTES)
        .build()
        .map_err(Error::Thread)?;
    runtime.block_on(async move {
        tokio::spawn(run::run(hive, request, events))
            .await
            .map_err(|_| Error::ThreadPanicked)?
    })
}

#[cfg(test)]
mod test;
