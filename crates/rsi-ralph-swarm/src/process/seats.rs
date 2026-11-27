//! Starting and stopping process seats with `tinyhivemind-deploy`, which
//! runs on Unix hosts only.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use tinyhivemind_deploy::{Deployment, ProcessSeat, ProcessSeats};
use tinyhivemind_exec::ExecContext;
use tinyhivemind_hives::Coordinator;
use tinyhivemind_lang::{
    Access, HarnessKind, LoweredModel, LoweredSeat, LoweredSeatIntegration, Model, Provider,
    ProviderKind,
};
use tinyhivemind_mcp::{SeatTools, Services};

use super::{HARNESS_DIR, harness, tools};
use crate::event::SeatInfo;
use crate::package::{Hive, SeatPlan};
use crate::prompt::{PromptParts, system_prompt};
use crate::swarm::{SwarmRequest, action_dir};
use crate::{Error, Result};

/// A process seat laid out but not started.
pub(crate) type Pending = ProcessSeat;
/// The started process seats of one run.
pub(crate) type Running = ProcessSeats;

/// How long an `OpenHuman` process may take to start: its first start on a
/// cold machine is slow.
const OPENHUMAN_READY: Duration = Duration::from_secs(60);

/// Refuses what process seat `plan` cannot have: seat memory, tool keys,
/// and an executable tool named like one of the host tools.
pub(super) fn validate(plan: &SeatPlan, request: &SwarmRequest) -> Result<()> {
    let seat = &plan.seat.id;
    let unsupported = |what: &'static str| Error::HarnessUnsupported {
        seat: seat.clone(),
        what,
    };
    if request.memory.is_some() {
        return Err(unsupported("seat memory"));
    }
    if !crate::tools::for_seat(&request.tools, seat).is_empty() {
        return Err(unsupported("tool keys"));
    }
    for program in &plan.seat.integration.programs {
        if tools::STANDARD.iter().any(|tool| tool.id == program.id) {
            return Err(Error::ToolNameClash {
                seat: seat.clone(),
                tool: program.id.clone(),
            });
        }
    }
    Ok(())
}

/// `plan`'s process seat, and its announcement.
pub(crate) fn seat(
    plan: &SeatPlan,
    request: &SwarmRequest,
    hive: &Hive,
) -> Option<(Pending, SeatInfo)> {
    let harness = harness(plan, request)?;
    let id = plan.seat.id.clone();
    let action_dir = action_dir(plan, request);
    let prompt = system_prompt(&PromptParts {
        seat: &plan.seat.prompt,
        role: &plan.role,
        charter: &plan.charter,
        action_dir,
        editing: plan.editing,
        context: &hive.lowered.context,
    });
    let model = plan.seat.integration.model.as_ref().map_or_else(
        || request.default_model.clone(),
        |model| model.model.name.clone(),
    );
    let url = (request.model_base_url)(&id);
    // Claude Code takes the API root and adds `/v1/messages` itself.
    let (kind, endpoint) = match harness.kind {
        HarnessKind::ClaudeCode => (
            ProviderKind::Anthropic,
            url.strip_suffix("/v1").unwrap_or(&url).to_owned(),
        ),
        _ => (ProviderKind::Openai, url),
    };
    let provider = Provider {
        id: "gateway".into(),
        kind,
        endpoint: Some(endpoint),
        credential: None,
    };
    let lowered = LoweredSeat {
        id: id.clone(),
        label: plan.seat.label.clone(),
        template: plan.seat.template.clone(),
        config: serde_json::Value::Null,
        prompt,
        memory: None,
        integration: LoweredSeatIntegration {
            model: Some(LoweredModel {
                model: Model {
                    id: "model".into(),
                    provider: Some("gateway".into()),
                    name: model.clone(),
                    ..Model::default()
                },
                provider: Some(provider),
            }),
            harness: Some(harness.clone()),
            ..LoweredSeatIntegration::default()
        },
    };
    let integration = &plan.seat.integration;
    let mut programs: Vec<_> = tools::granted(
        &integration.tools.allow,
        &integration.tools.deny,
        plan.editing,
    )
    .into_iter()
    .map(tools::declaration)
    .collect();
    programs.extend(integration.programs.iter().cloned());
    let context = ExecContext {
        package_root: request.run_dir.clone(),
        workspace: Some(action_dir.to_path_buf()),
        hive: hive.lowered.id.clone(),
        seat: id.clone(),
        features: BTreeMap::new(),
    };
    let access = if plan.editing {
        Access::Autonomous
    } else {
        Access::ReadOnly
    };
    let tools = SeatTools::new(id.clone())
        .with_programs(programs)
        .with_context(context)
        .with_access(access);
    let directory = request.run_dir.join(HARNESS_DIR).join("seats").join(&id);
    let info = SeatInfo {
        id: id.clone(),
        role: plan.role.clone(),
        model,
        action_dir: action_dir.to_path_buf(),
        editing: plan.editing,
        memory: None,
        harness: Some(harness.kind.to_string()),
    };
    Some((ProcessSeat::new(lowered, directory).with_tools(tools), info))
}

/// Starts `seats` on their harnesses and registers each with `coordinator`;
/// `None` when there are none.
pub(crate) async fn start(
    coordinator: &Coordinator,
    seats: Vec<Pending>,
    request: &SwarmRequest,
    run_dir: &Path,
) -> Result<Option<Running>> {
    if seats.is_empty() {
        return Ok(None);
    }
    let mut deployment = Deployment::new(run_dir.join(HARNESS_DIR));
    deployment.openhuman.ready = OPENHUMAN_READY;
    let services = Services::new(coordinator.clone());
    let started = ProcessSeats::start(
        coordinator,
        services,
        seats,
        &request.harnesses.pinned(),
        &deployment,
    )
    .await
    .map_err(Error::Harness)?;
    Ok(Some(started))
}

/// Stops `seats`, if any started.
pub(crate) async fn stop(seats: Option<&mut Running>) -> Result<()> {
    match seats {
        Some(seats) => seats.stop().await.map_err(Error::Harness),
        None => Ok(()),
    }
}
