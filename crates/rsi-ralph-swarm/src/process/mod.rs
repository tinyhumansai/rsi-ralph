//! Seats that run on a process harness: `OpenHuman`, Claude Code, Codex or
//! `OpenClaw` as a separate program, started by `tinyhivemind-deploy`.
//!
//! A seat runs on a process when the request's `seat_harnesses` names one
//! for it (or, for a package written for process harnesses, its own entry
//! does); every other seat runs in-process as before. A process seat gets
//! the prompt an in-process seat gets, its model through the seat's own
//! model URL (the spend gateway's tagged URL, in the API its harness
//! speaks), and the host tools of `tools` in its checkout. Its package `config`, tool lists and
//! model settings are the swarm's to honour, so the lowered seat the harness
//! receives carries only the prompt, the model and the harness.
//!
//! What only an in-process seat has is refused before anything starts: seat
//! memory and tool keys (web search). Process harnesses run on Unix hosts
//! only: elsewhere every process seat is refused the same way. A process
//! seat's turns are not seen by the swarm's hooks either, so its
//! [`SeatUsage`] is left incomplete; the gateway's meter has its calls.

mod installs;
#[cfg(unix)]
mod seats;
#[cfg(unix)]
pub(crate) mod tools;
#[cfg(not(unix))]
mod unsupported;

pub use installs::{
    CLAUDE_CODE_ENV, CLAUDE_CODE_VERSION, CODEX_ACP_ENV, CODEX_ACP_VERSION, CODEX_ENV,
    HarnessInstalls, NODE_ENV, OPENCLAW_ENV, OPENCLAW_VERSION, OPENHUMAN_ENV, OPENHUMAN_VERSION,
    harness_installs,
};
#[cfg(unix)]
pub(crate) use seats::{Pending, seat, start, stop};
#[cfg(not(unix))]
pub(crate) use unsupported::{Pending, seat, start, stop};

use tinyhivemind_lang::Harness;

use crate::package::{Hive, SeatPlan};
use crate::swarm::{SeatUsage, SwarmRequest};
use crate::{Error, Result};

/// Where the harness processes keep their state, under the run directory.
pub const HARNESS_DIR: &str = "harnesses";

/// The process harness `plan`'s seat runs on, if any: the request's choice,
/// else the package's own.
pub(crate) fn harness(plan: &SeatPlan, request: &SwarmRequest) -> Option<Harness> {
    match request.seat_harnesses.get(&plan.seat.id) {
        Some(kind) => Some(Harness {
            id: kind.to_string(),
            kind: *kind,
            isolation: None,
        }),
        None => plan.seat.integration.harness.clone(),
    }
}

/// Whether `plan`'s seat runs on a process harness.
pub(crate) fn is_process(plan: &SeatPlan, request: &SwarmRequest) -> bool {
    harness(plan, request).is_some()
}

/// Refuses a harness for a seat the package lacks, and what a process seat
/// cannot have: seat memory, tool keys, and an executable tool named like
/// one of the host tools. Off Unix, refuses every process seat.
pub(crate) fn validate(hive: &Hive, request: &SwarmRequest) -> Result<()> {
    for seat in request.seat_harnesses.keys() {
        if !hive.seats.iter().any(|plan| &plan.seat.id == seat) {
            return Err(Error::UnknownHarnessSeat(seat.clone()));
        }
    }
    let mut process = hive.seats.iter().filter(|plan| is_process(plan, request));
    #[cfg(not(unix))]
    if let Some(plan) = process.next() {
        return Err(Error::HarnessPlatform(plan.seat.id.clone()));
    }
    #[cfg(unix)]
    process.try_for_each(|plan| seats::validate(plan, request))?;
    Ok(())
}

/// A process seat's tally: the swarm's hooks see none of its turns, so its
/// usage is unknown rather than zero.
pub(crate) fn unknown_usage() -> SeatUsage {
    SeatUsage {
        cost_complete: false,
        ..SeatUsage::default()
    }
}

#[cfg(all(test, unix))]
mod test;
