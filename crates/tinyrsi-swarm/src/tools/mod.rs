//! Provider keys for seat tools: which capabilities exist, and how a key
//! reaches the seats it is scoped to.
//!
//! The one capability today is [`WEB_SEARCH`]: `OpenHuman`'s `TinySearch`
//! module, presented as the role tools in [`SEARCH_TOOLS`]. A key is applied
//! to each scoped seat's own agent config (never the runtime's, never a
//! file), switching that provider to its direct route, and the seat's allow
//! list gains the search tools.
//!
//! The key is pinned as an in-memory `SearchHostCredential`
//! (`SearchConfig::pin_host_credential`): it is never written to the saved
//! `api_key` fields, never serialized, and prints redacted. `OpenHuman`'s
//! search proxy uses a pinned config as given and reconfigures its shared
//! module per caller, so two seats with different keys never see each
//! other's.

mod types;

use std::collections::BTreeSet;

use openhuman_embed::{RuntimeConfig, SearchHostCredential};
pub use types::ToolKey;

use crate::{Error, Result};

/// The web-search capability, backed by `OpenHuman`'s `TinySearch` module.
pub const WEB_SEARCH: &str = "web_search";

/// Search providers a [`WEB_SEARCH`] key can name: those `TinySearch` reaches
/// on a direct route with a single API key.
pub const SEARCH_PROVIDERS: [&str; 7] = [
    "brave", "exa", "gemini", "parallel", "querit", "tavily", "tinyfish",
];

/// The tools a seat holding a [`WEB_SEARCH`] key is allowed: ranked results,
/// a grounded and cited answer (Gemini's grounding lives here), and page
/// contents.
pub const SEARCH_TOOLS: [&str; 3] = ["web_search_tool", "web_answer_tool", "web_contents_tool"];

/// Whether this build carries `OpenHuman`'s module loader, which search needs.
const SEARCH_COMPILED_IN: bool = cfg!(feature = "web-search");

/// Checks every key against the capabilities this build supports and the
/// package's seats.
pub(crate) fn validate(keys: &[ToolKey], seats: &BTreeSet<&str>) -> Result<()> {
    for key in keys {
        let supported = key.capability == WEB_SEARCH
            && SEARCH_PROVIDERS.contains(&key.provider.as_str())
            && SEARCH_COMPILED_IN;
        if !supported {
            return Err(Error::UnsupportedTool {
                capability: key.capability.clone(),
                provider: key.provider.clone(),
            });
        }
        for seat in key.seats.iter().flatten() {
            if !seats.contains(seat.as_str()) {
                return Err(Error::UnknownToolSeat {
                    capability: key.capability.clone(),
                    seat: seat.clone(),
                });
            }
        }
    }
    Ok(())
}

/// The keys that reach `seat`.
pub(crate) fn for_seat(keys: &[ToolKey], seat: &str) -> Vec<ToolKey> {
    keys.iter()
        .filter(|key| key.reaches(seat))
        .cloned()
        .collect()
}

/// The tool names `keys` add to a seat's allow list.
pub(crate) fn tool_names(keys: &[ToolKey]) -> Vec<String> {
    if keys.iter().any(|key| key.capability == WEB_SEARCH) {
        SEARCH_TOOLS.iter().map(|tool| (*tool).to_owned()).collect()
    } else {
        Vec::new()
    }
}

/// Pins `keys` into one seat's agent config.
///
/// A provider `OpenHuman` cannot pin is skipped; [`validate`] has already
/// refused every such key before a seat is built.
pub(crate) fn apply(config: &mut RuntimeConfig, keys: &[ToolKey]) {
    for key in keys.iter().filter(|key| key.capability == WEB_SEARCH) {
        let credential = SearchHostCredential::new(key.secret.clone());
        // Unreachable after `validate`; skipping keeps the seat buildable.
        let _ = config.search.pin_host_credential(&key.provider, credential);
    }
}

#[cfg(test)]
mod test;
