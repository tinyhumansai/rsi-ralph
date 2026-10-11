//! The evolution port: given the incumbent and the goal, propose changes.
//!
//! An evolver proposes *changes*, not new subjects. What it reads (`S`) and
//! what it writes ([`Evolver::Proposal`]) are different types: the script
//! supervisor reads a brief and writes file replacements; retreat reads a hive
//! package and writes typed patches. Turning a proposal into a candidate stays
//! with the loop that owns the subject.
use crate::Result;
use tinyrsi_types::Goal;

/// Where in the loop a proposal is being asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvolveContext {
    /// Epoch number, from 1.
    pub epoch: u64,
    /// Index of this request within the epoch, from 0.
    pub index: u32,
    /// Seed for any sampling the evolver does.
    pub seed: u64,
}

/// What one call produced: measured tokens and proposals in rank order.
///
/// A proposal the evolver produced but could not validate is an `Err`; the
/// loop records it and continues past it.
#[derive(Debug, PartialEq)]
pub struct Proposed<P> {
    /// Measured model tokens spent on this call.
    pub tokens: u64,
    /// Proposals, best first when the evolver ranks them.
    pub proposals: Vec<Result<P>>,
}

/// Proposes changes to an incumbent.
pub trait Evolver<S: ?Sized> {
    /// What this evolver proposes.
    type Proposal;

    /// Proposes changes to `incumbent` toward `goal`.
    ///
    /// # Errors
    ///
    /// Returns an error when the evolver could not run at all, for example a
    /// failed model call. Individual unusable proposals are not errors; they
    /// are `Err` entries in [`Proposed::proposals`].
    fn propose(
        &self,
        incumbent: &S,
        goal: &Goal,
        context: &EvolveContext,
    ) -> Result<Proposed<Self::Proposal>>;
}

#[cfg(test)]
mod test;
