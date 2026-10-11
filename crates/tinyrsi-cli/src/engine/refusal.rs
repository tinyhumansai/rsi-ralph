//! Provider refusals during an epoch's hive run.
//!
//! A hive whose every model call the provider refuses (a key over its daily
//! limit answers `403`) changes nothing, which looks exactly like a hive with
//! no ideas. The engine reads the meter's upstream calls around the hive and,
//! when refusals explain an empty epoch, says so: a `provider_refused` trace
//! event, plus a run warning, or the run's error and stop reason when every
//! call was refused.

use tinyrsi_gateway::UpstreamCalls;
use tinyrsi_trace::LoopEvent;

use crate::store;

/// Refusals that explain an epoch.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Refused {
    /// Every call was refused, so the run cannot make progress.
    pub(super) all: bool,
    /// One line for the operator, such as `provider refused 2 of 2 model
    /// calls (403: Key limit exceeded)`.
    pub(super) line: String,
    /// The trace event recording them.
    pub(super) event: LoopEvent,
}

/// What `calls`, the hive's calls in `epoch`, say about that epoch, given
/// whether any candidate changed. Refusals are reported when the epoch
/// produced nothing, or when the provider refused every call; otherwise a
/// few refused calls the hive worked around are not news. The provider's
/// message is scrubbed of `secrets`.
pub(super) fn explain<'a>(
    epoch: u32,
    calls: &UpstreamCalls,
    changed: bool,
    secrets: impl IntoIterator<Item = &'a str>,
) -> Option<Refused> {
    let all = calls.all_refused();
    if calls.refused == 0 || (changed && !all) {
        return None;
    }
    let (status, message) = calls
        .last_refusal
        .as_ref()
        .map_or((0, String::new()), |r| (r.status, store::scrub(&r.message, secrets)));
    let line = format!(
        "provider refused {} of {} model calls ({status}: {message})",
        calls.refused, calls.sent
    );
    Some(Refused {
        all,
        line,
        event: LoopEvent::ProviderRefused {
            epoch: epoch.into(),
            calls: u32::try_from(calls.sent).unwrap_or(u32::MAX),
            refused: u32::try_from(calls.refused).unwrap_or(u32::MAX),
            status,
            message,
        },
    })
}
