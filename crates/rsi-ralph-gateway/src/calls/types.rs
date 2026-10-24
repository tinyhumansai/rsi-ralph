//! The public view of upstream call outcomes.

/// A point in a gateway's sequence of upstream calls: the number of calls
/// dispatched before it. Take one with
/// [`Gateway::call_mark`](crate::Gateway::call_mark) before a phase and pass
/// it to [`Gateway::upstream_calls_since`](crate::Gateway::upstream_calls_since)
/// after, to see that phase's calls alone. The default mark is the start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallMark(pub u64);

/// What the provider made of a run of upstream calls.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpstreamCalls {
    /// Calls forwarded to the provider, counted as they are sent, so a
    /// call still waiting on its answer is included.
    pub sent: u64,
    /// Of those, calls refused with a non-success status or never answered
    /// completely, as far as is known so far.
    pub refused: u64,
    /// The latest refusal among them, if any.
    pub last_refusal: Option<Refusal>,
}

impl UpstreamCalls {
    /// Whether at least one call was sent and the provider refused them
    /// all.
    #[must_use]
    pub fn all_refused(&self) -> bool {
        self.sent > 0 && self.refused >= self.sent
    }
}

/// One provider refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// HTTP status the provider answered with; `502` when it could not be
    /// reached or its answer broke off.
    pub status: u16,
    /// The provider's message, on one line, free of the provider key.
    pub message: String,
}
