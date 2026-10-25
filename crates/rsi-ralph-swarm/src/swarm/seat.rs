//! One seat's `OpenHuman` agent specification.

use std::collections::BTreeMap;
use std::path::Path;

use openhuman_embed::{
    Access, AgentDefinitionSpec, AgentSpec, ModelDefaults, Provider, ToolScopeSpec,
};

use super::SwarmRequest;
use crate::event::SeatInfo;
use crate::package::SeatPlan;
use crate::prompt::{PromptParts, system_prompt};
use crate::{programs, tools};

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

/// `OpenHuman`'s memory tool, added to an explicit allow list when seats
/// carry memory, so a seat can recall and record on purpose as well as
/// through the per-turn lifecycle.
pub(crate) const MEMORY_TOOL: &str = "memory";

/// Where `plan`'s seat works: its candidate checkout when it edits one, the
/// incumbent otherwise.
pub(crate) fn action_dir<'a>(plan: &SeatPlan, request: &'a SwarmRequest) -> &'a Path {
    if plan.editing {
        // `check` guarantees every editing seat has a candidate.
        request
            .candidates
            .get(&plan.seat.id)
            .map_or(request.incumbent.as_path(), |path| path.as_path())
    } else {
        request.incumbent.as_path()
    }
}

/// A seat's agent specification and its announcement.
pub(crate) fn spec(
    plan: &SeatPlan,
    request: &SwarmRequest,
    context: &BTreeMap<String, String>,
) -> (AgentSpec, SeatInfo) {
    let seat = &plan.seat;
    let action_dir = action_dir(plan, request);
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
        allow.extend(programs::names(plan));
        if request.memory.is_some() {
            allow.push(MEMORY_TOOL.to_owned());
        }
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
    // The hive model's completion cap. Without it OpenHuman asks for its own
    // per-turn cap (16,384) whatever the hive and the gateway allow.
    let model_defaults = ModelDefaults {
        max_tokens: seat
            .integration
            .model
            .as_ref()
            .and_then(|model| model.model.max_tokens),
        ..ModelDefaults::default()
    };
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
        .model_defaults(model_defaults)
        .provider(provider)
        .access(access)
        .action_dir(action_dir);
    if !keys.is_empty() {
        spec = spec.config(move |config| tools::apply(config, &keys));
    }
    if let Some(memory) = &request.memory {
        spec = spec.memory(memory.binding(&seat.id));
    }
    let info = SeatInfo {
        id: seat.id.clone(),
        role: plan.role.clone(),
        model,
        action_dir: action_dir.to_path_buf(),
        editing: plan.editing,
        memory: request
            .memory
            .as_ref()
            .map(|memory| memory.namespace(&seat.id)),
    };
    (spec, info)
}
