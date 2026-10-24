//! One seat's `OpenHuman` agent specification.

use std::collections::BTreeMap;
use std::path::Path;

use openhuman_embed::{Access, AgentDefinitionSpec, AgentSpec, Provider, ToolScopeSpec};

use super::SwarmRequest;
use crate::event::SeatInfo;
use crate::package::SeatPlan;
use crate::prompt::{PromptParts, system_prompt};
use crate::tools;

/// The permanent tools `tinyhivemind-openhuman` attaches to every seat. A
/// seat with an explicit allow list must keep them, or it cannot talk.
pub(crate) const HIVEMIND_TOOLS: [&str; 9] = [
    "hivemind_ask",
    "hivemind_broadcast",
    "hivemind_complete",
    "hivemind_list_agents",
    "hivemind_list_hives",
    "hivemind_post",
    "hivemind_read",
    "hivemind_send_agent",
    "hivemind_send_hive",
];

/// Tools every seat is denied. Turns are asynchronous: a seat that waits or
/// polls inside its turn holds its coordinator slot until the turn wall, and
/// the peer it waits for is never scheduled.
pub(crate) const DENIED_TOOLS: [&str; 2] = ["wait", "wait_loop"];

/// A seat's agent specification and its announcement.
pub(crate) fn spec(
    plan: &SeatPlan,
    request: &SwarmRequest,
    context: &BTreeMap<String, String>,
) -> (AgentSpec, SeatInfo) {
    let seat = &plan.seat;
    let action_dir: &Path = if plan.editing {
        // `check` guarantees every editing seat has a candidate.
        request
            .candidates
            .get(&seat.id)
            .map_or(request.incumbent.as_path(), |path| path.as_path())
    } else {
        request.incumbent.as_path()
    };
    let keys = tools::for_seat(&request.tools, &seat.id);
    let prompt = system_prompt(&PromptParts {
        seat: &seat.prompt,
        role: &plan.role,
        charter: &plan.charter,
        action_dir,
        editing: plan.editing,
        context,
    });
    let mut definition = AgentDefinitionSpec::new().system_prompt(prompt);
    if !seat.integration.tools.allow.is_empty() {
        let mut allow = seat.integration.tools.allow.clone();
        allow.extend(HIVEMIND_TOOLS.iter().map(|tool| (*tool).to_owned()));
        allow.extend(tools::tool_names(&keys));
        allow.sort();
        allow.dedup();
        definition = definition.tools(ToolScopeSpec::Named(allow));
    }
    let mut deny = seat.integration.tools.deny.clone();
    deny.extend(DENIED_TOOLS.iter().map(|tool| (*tool).to_owned()));
    deny.sort();
    deny.dedup();
    definition = definition.disallow_tools(deny);
    if let Some(iterations) = seat.config["max_iterations"].as_u64() {
        definition = definition.max_iterations(usize::try_from(iterations).unwrap_or(usize::MAX));
    }
    if let Some(temperature) = seat.config["temperature"].as_f64().or(request.temperature) {
        definition = definition.temperature(temperature);
    }
    let model = seat.integration.model.as_ref().map_or_else(
        || request.default_model.clone(),
        |model| model.model.name.clone(),
    );
    let provider = Provider::openai_compatible(
        (request.model_base_url)(&seat.id),
        request.model_api_key.clone(),
    )
    .model(model.clone());
    let access = if plan.editing {
        Access::full()
    } else {
        Access::readonly()
    };
    let mut spec = AgentSpec::new(seat.id.clone())
        .definition(definition)
        .provider(provider)
        .access(access)
        .action_dir(action_dir);
    if !keys.is_empty() {
        spec = spec.config(move |config| tools::apply(config, &keys));
    }
    let info = SeatInfo {
        id: seat.id.clone(),
        role: plan.role.clone(),
        model,
        action_dir: action_dir.to_path_buf(),
        editing: plan.editing,
    };
    (spec, info)
}
