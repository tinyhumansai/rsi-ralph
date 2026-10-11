//! An evolver that replays canned proposals, for tests and offline examples.
use crate::{Error, EvolveContext, Evolver, Proposed, Result};
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::Mutex;
use tinyrsi_types::Goal;

/// Replays queued [`Proposed`] batches in order, then fails.
///
/// The incumbent and goal are ignored, so the run is fully determined by the
/// queue. Running out is [`Error::Exhausted`], which a loop treats like any
/// failed evolver call.
#[derive(Debug)]
pub struct ScriptedEvolver<S: ?Sized, P> {
    queue: Mutex<VecDeque<Proposed<P>>>,
    subject: PhantomData<fn(&S)>,
}

impl<S: ?Sized, P> ScriptedEvolver<S, P> {
    /// Replays `batches` in order.
    #[must_use]
    pub fn new(batches: impl IntoIterator<Item = Proposed<P>>) -> Self {
        Self {
            queue: Mutex::new(batches.into_iter().collect()),
            subject: PhantomData,
        }
    }

    /// Replays one proposal per call, each costing no tokens.
    #[must_use]
    pub fn one_each(proposals: impl IntoIterator<Item = P>) -> Self {
        Self::new(proposals.into_iter().map(|p| Proposed {
            tokens: 0,
            proposals: vec![Ok(p)],
        }))
    }

    /// Batches not yet replayed.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.queue.lock().map_or(0, |q| q.len())
    }
}

impl<S: ?Sized, P> Evolver<S> for ScriptedEvolver<S, P> {
    type Proposal = P;

    fn propose(&self, _: &S, _: &Goal, _: &EvolveContext) -> Result<Proposed<P>> {
        self.queue
            .lock()
            .map_err(|_| Error::Exhausted)?
            .pop_front()
            .ok_or(Error::Exhausted)
    }
}

#[cfg(test)]
mod test;
