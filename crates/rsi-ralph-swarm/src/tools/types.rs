//! A provider key for one seat tool capability.

/// A provider key for a tool capability, scoped to some or all seats.
///
/// The secret is applied to the scoped seats' in-memory agent config only:
/// it is never written to the run directory, never appears in an event, and
/// is hidden from this type's `Debug` output.
#[derive(Clone, PartialEq, Eq)]
pub struct ToolKey {
    /// The capability, e.g. [`crate::WEB_SEARCH`].
    pub capability: String,
    /// The provider backing it, e.g. `gemini`; see [`crate::SEARCH_PROVIDERS`].
    pub provider: String,
    /// The provider API key.
    pub secret: String,
    /// The seats that get the capability; `None` means every seat.
    pub seats: Option<Vec<String>>,
}

impl ToolKey {
    /// Whether this key reaches `seat`.
    #[must_use]
    pub fn reaches(&self, seat: &str) -> bool {
        self.seats
            .as_ref()
            .is_none_or(|seats| seats.iter().any(|scoped| scoped == seat))
    }
}

impl std::fmt::Debug for ToolKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolKey")
            .field("capability", &self.capability)
            .field("provider", &self.provider)
            .field("secret", &crate::event::REDACTED)
            .field("seats", &self.seats)
            .finish()
    }
}
