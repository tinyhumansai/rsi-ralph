//! Host transport boundary and counted decoding results.
use crate::Result;
use serde::Serialize;
use serde_json::Value;
use tinyrsi_types::Proposal;

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
