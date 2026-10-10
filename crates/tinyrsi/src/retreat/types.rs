//! Host transport boundary and counted decoding results.
use crate::{Proposal, Result};
use serde_json::Value;

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
