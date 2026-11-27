//! Process seats off Unix: `tinyhivemind-deploy` runs on Unix hosts only,
//! so [`super::validate`] refuses every process seat and none is ever laid
//! out, started or stopped.

use std::future::Ready;
use std::path::Path;

use tinyhivemind_hives::Coordinator;

use crate::Result;
use crate::event::SeatInfo;
use crate::package::{Hive, SeatPlan};
use crate::swarm::SwarmRequest;

/// A process seat laid out but not started: none can be.
pub(crate) enum Pending {}
/// The started process seats of one run: none can be.
pub(crate) enum Running {}

/// No seat runs on a process here.
pub(crate) fn seat(
    _plan: &SeatPlan,
    _request: &SwarmRequest,
    _hive: &Hive,
) -> Option<(Pending, SeatInfo)> {
    None
}

/// Starts nothing: `seats` is always empty.
pub(crate) fn start(
    _coordinator: &Coordinator,
    _seats: Vec<Pending>,
    _request: &SwarmRequest,
    _run_dir: &Path,
) -> Ready<Result<Option<Running>>> {
    std::future::ready(Ok(None))
}

/// Stops nothing: no seat ever started.
pub(crate) fn stop(_seats: Option<&mut Running>) -> Ready<Result<()>> {
    std::future::ready(Ok(()))
}
