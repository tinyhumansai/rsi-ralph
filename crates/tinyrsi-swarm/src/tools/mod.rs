//! Provider keys for seat tools: which capabilities exist, and how a key
//! reaches the seats it is scoped to.
//!
//! The one capability today is [`WEB_SEARCH`]: `OpenHuman`'s `TinySearch`
//! module, presented as the role tools in [`SEARCH_TOOLS`]. A key is applied
//! to each scoped seat's own agent config (never the runtime's, never a
//! file), switching that provider to its direct route, and the seat's allow
//! list gains the search tools.
//!
//! # Known upstream limitation
//!
//! `OpenHuman` (pinned `vendor/openhuman`) offers the tools from the agent's
//! config, but its search proxy re-reads `config.toml` and the process
//! environment before every call (`modules/search/proxy.rs`), and runs one
//! process-wide module. A per-seat key therefore selects the tools without
//! yet authenticating their calls; they answer "web search is not
//! available" until the proxy uses the config it is given. The wiring here
//! is the intended seam and needs no change when that is fixed upstream.

mod types;

use std::collections::BTreeSet;

use openhuman_embed::RuntimeConfig;
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

/// Writes `keys` into one seat's agent config.
pub(crate) fn apply(config: &mut RuntimeConfig, keys: &[ToolKey]) {
    for key in keys {
        if key.capability != WEB_SEARCH {
            continue;
        }
        let search = &mut config.search;
        let credentials = match key.provider.as_str() {
            "brave" => &mut search.brave,
            "exa" => &mut search.exa,
            "gemini" => &mut search.gemini,
            "parallel" => &mut search.parallel,
            "querit" => &mut search.querit,
            "tavily" => &mut search.tavily,
            "tinyfish" => &mut search.tinyfish,
            _ => continue,
        };
        credentials.api_key = Some(key.secret.clone());
        search.enabled = Some(true);
        // The default settings are enabled on the direct route, which is
        // the one a bring-your-own key authenticates.
        search
            .providers
            .insert(key.provider.clone(), Default::default());
    }
}

#[cfg(test)]
mod test;
