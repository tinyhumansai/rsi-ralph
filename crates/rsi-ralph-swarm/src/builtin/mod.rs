//! The tool names `OpenHuman` registers on every seat before the hive's own
//! tools are attached, and the check that a hive tool does not reuse one.
//!
//! `Agent::attach_tools` refuses a tool whose name is already taken, but only
//! when the seat's agent is built, which is the first epoch of a run. The
//! names it compares against are the registry `OpenHuman` lists for the
//! agent's config plus the tools it assembles for that config; [`names`] asks
//! the same two functions, over the default config the swarm's runtime starts
//! from, so the list is `OpenHuman`'s own and not a copy that drifts. The
//! search tools a `web_search` key adds ([`SEARCH_TOOLS`]) are counted too,
//! whether or not a key is present, so a package never depends on which keys
//! a run holds.

use std::collections::BTreeSet;
use std::sync::Arc;

use openhuman_core::config::Config;
use openhuman_core::security::{AuditLogger, SecurityPolicy};
use openhuman_core::tools::ops::all_tools;
use openhuman_core::tools::registry::ops::registry_entries_for_config;

use crate::package::Hive;
use crate::tools::SEARCH_TOOLS;
use crate::{Error, Result};

/// Every tool name `OpenHuman` registers on a seat itself.
pub(crate) fn names() -> BTreeSet<String> {
    let config = Config::default();
    let security = Arc::new(SecurityPolicy::from_config(
        &config.autonomy,
        &config.workspace_dir,
        &config.action_dir,
    ));
    let mut names: BTreeSet<String> = registry_entries_for_config(&config)
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    names.extend(
        all_tools(
            Arc::new(config.clone()),
            &security,
            AuditLogger::disabled(),
            &config.browser,
            &config.http_request,
            &config.action_dir,
            &config,
        )
        .iter()
        .map(|tool| tool.name().to_string()),
    );
    names.extend(SEARCH_TOOLS.iter().map(|tool| (*tool).to_owned()));
    names
}

/// Refuses a package whose seats declare an executable tool named like one
/// `OpenHuman` registers itself.
///
/// # Errors
///
/// [`Error::ToolNameClash`] naming the first such seat and tool.
pub(crate) fn validate(hive: &Hive) -> Result<()> {
    let taken = names();
    for plan in &hive.seats {
        if let Some(tool) = plan
            .seat
            .integration
            .programs
            .iter()
            .find(|tool| taken.contains(&tool.id))
        {
            return Err(Error::ToolNameClash {
                seat: plan.seat.id.clone(),
                tool: tool.id.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod test;
