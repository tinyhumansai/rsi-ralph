//! The public view of upstream call outcomes.

/// What the provider made of the calls forwarded to it, read in-process
/// with [`Gateway::upstream_calls`](crate::Gateway::upstream_calls).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpstreamCalls {
    /// Calls forwarded to the provider.
    pub sent: u64,
    /// Of those, calls refused with a non-success status or never answered.
    pub refused: u64,
    /// The latest refusal, if any.
    pub last_refusal: Option<Refusal>,
}

impl UpstreamCalls {
    /// The calls made after `earlier`, an older reading of the same
    /// gateway. The latest refusal is kept only when one happened since.
    #[must_use]
    pub fn since(&self, earlier: &Self) -> Self {
        let refused = self.refused.saturating_sub(earlier.refused);
        Self {
            sent: self.sent.saturating_sub(earlier.sent),
            refused,
            last_refusal: if refused > 0 {
                self.last_refusal.clone()
            } else {
                None
            },
        }
    }

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
    /// reached.
    pub status: u16,
    /// The provider's message, on one line, free of the provider key.
    pub message: String,
}
