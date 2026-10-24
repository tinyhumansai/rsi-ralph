//! Retreat behind the evolution port.
use super::{RetreatDossier, RetreatTransport, build_retreat_request, decode_retreat_response};
use crate::{Error, EvolveContext, Evolver, Proposed, Result};
use rsi_ralph_types::{Goal, Package, Proposal, ProposalEvent};

/// What retreat reads: the hive, its public failure dossier, the incumbent
/// package and the attempts already rejected.
#[derive(Debug, Clone)]
pub struct RetreatBrief<D> {
    /// Hive identity; scopes the engine's memory namespace.
    pub hive_id: String,
    /// Public failure evidence for the epoch.
    pub dossier: D,
    /// The incumbent package the proposals edit.
    pub incumbent: Package,
    /// Earlier attempts, shown to the engine as prior ideas.
    pub rejected: Vec<ProposalEvent>,
}

/// Runs the tinybackrooms retreat through a host transport.
///
/// Proposals come back in the engine's rank order. Items the engine returned
/// but that do not decode into a valid typed [`Proposal`] become
/// [`Error::InvalidProposal`] entries, after the valid ones. The transport
/// reports no token usage, so `tokens` is zero; meter the transport itself.
#[derive(Debug)]
pub struct RetreatEvolver<T> {
    transport: T,
}

impl<T> RetreatEvolver<T> {
    /// Uses `transport` to reach the engine.
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T: RetreatTransport, D: RetreatDossier> Evolver<RetreatBrief<D>> for RetreatEvolver<T> {
    type Proposal = Proposal;

    /// # Errors
    ///
    /// Returns [`Error::InvalidProposal`] for an unsafe request or a response
    /// with no items, and the transport's own error when it fails.
    fn propose(
        &self,
        brief: &RetreatBrief<D>,
        _: &Goal,
        _: &EvolveContext,
    ) -> Result<Proposed<Proposal>> {
        let request = build_retreat_request(
            &brief.hive_id,
            &brief.dossier,
            &brief.incumbent,
            &brief.rejected,
        )?;
        let output = decode_retreat_response(&self.transport.generate(&request)?)?;
        if output.proposals.is_empty() && output.invalid_items == 0 {
            return Err(Error::InvalidProposal);
        }
        let mut proposals: Vec<Result<Proposal>> = output.proposals.into_iter().map(Ok).collect();
        proposals.extend((0..output.invalid_items).map(|_| Err(Error::InvalidProposal)));
        Ok(Proposed {
            tokens: 0,
            proposals,
        })
    }
}
