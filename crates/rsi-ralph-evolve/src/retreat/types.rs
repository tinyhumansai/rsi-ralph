//! Host transport boundary and counted decoding results.
use crate::Result;
use serde::Serialize;
use serde_json::Value;
use rsi_ralph_types::Proposal;

/// The public failure evidence retreat reads: any serializable dossier that
/// knows its epoch. The `tinyrsi` crate implements it for its `Dossier`; the
/// request carries the dossier's JSON and seeds the engine with the epoch.
pub trait RetreatDossier: Serialize {
    /// Host epoch the dossier describes.
    fn epoch(&self) -> u64;
}

/// Host-owned HTTP transport for tinybackrooms generation.
///
/// An implementation sends `request` unchanged as JSON to `POST /v1/generate`
/// and returns its decoded JSON response. Authentication, timeout, model backend
/// and persistent tinybackrooms memory are configured by the host.
pub trait RetreatTransport {
    /// Executes one tinybackrooms request.
    ///
    /// # Errors
    /// Returns [`crate::Error::HostFailed`] for transport or service failures.
    fn generate(&self, request: &Value) -> Result<Value>;
}

/// Valid typed proposals plus the count of discarded malformed service items.
#[derive(Clone, Debug, PartialEq)]
pub struct RetreatOutput {
    /// Strictly decoded proposals, preserving service ranking.
    pub proposals: Vec<Proposal>,
    /// Items whose value could not satisfy the typed proposal contract.
    pub invalid_items: usize,
}

/// The coarse, redacted host outcome of an earlier attempt, as shown to the
/// proposer. It carries no delta, interval or patch content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorOutcome {
    /// The candidate failed runs its incumbent completed (`HARMFUL`).
    Crashed,
    /// The host measured a confident regression.
    Regressed,
    /// Evaluated without a confident gain or loss.
    Flat,
    /// Stopped by cooldown before evaluation.
    Cooled,
    /// Confirmed, but the human declined promotion.
    Denied,
}

/// One earlier attempt as a `prior_idea` seed: epoch, class, a whitelisted
/// reason, the operation count and a coarse outcome.
///
/// Fields are private so nothing else, such as patch bodies, rationale or
/// measured magnitudes, can reach the proposal model through a seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorIdea {
    pub(super) epoch: u64,
    pub(super) class: rsi_ralph_types::PatchClass,
    pub(super) reason: &'static str,
    pub(super) operation_count: usize,
    pub(super) outcome: Option<PriorOutcome>,
}
