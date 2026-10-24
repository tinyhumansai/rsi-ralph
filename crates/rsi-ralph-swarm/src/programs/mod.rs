//! A seat's executable tools: the programs its package declares in
//! `hive.json` `tools`, attached to every one of its turns.
//!
//! A package declares a tool as a program plus an argument schema
//! (tinyhivemind `docs/specs/executable-tools.md`) and gives it to seats
//! through a toolpack. The language lowers each seat's programs; this module
//! turns them into one [`HostTools`] source for the host, which adds a
//! `tinyhivemind_exec::ProgramTool` per program to the turns of the seat
//! that declared it, made visible. Each call runs in the seat's working
//! directory, its candidate checkout or the incumbent, with the cleared
//! environment the convention specifies and `env` references resolved from
//! this process's environment.
//!
//! The swarm is handed a package as a path → text map, with no directory on
//! disk, so a program that is itself a package file (`tools/…`) has nothing
//! to run; [`validate`] refuses it before the run starts. A program on
//! `PATH` or named by an absolute path works.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use openhuman_embed::{HostTools, HostTurnTools};
use tinyhivemind_exec::{EnvSecrets, ExecContext, ProgramTool, Secrets};
use tinyhivemind_lang::ExecTool;

use crate::package::{Hive, SeatPlan};
use crate::{Error, Result};

/// Refuses a package whose seats declare a program shipped as a package
/// file.
///
/// # Errors
///
/// [`Error::PackageProgram`] naming the first such seat and tool.
pub(crate) fn validate(hive: &Hive) -> Result<()> {
    for plan in &hive.seats {
        if let Some(tool) = plan
            .seat
            .integration
            .programs
            .iter()
            .find(|tool| tool.command.iter().any(|part| part.starts_with("tools/")))
        {
            return Err(Error::PackageProgram {
                seat: plan.seat.id.clone(),
                tool: tool.id.clone(),
            });
        }
    }
    Ok(())
}

/// The ids of `plan`'s programs, which its allow list must keep.
pub(crate) fn names(plan: &SeatPlan) -> impl Iterator<Item = String> + '_ {
    plan.seat
        .integration
        .programs
        .iter()
        .map(|tool| tool.id.clone())
}

/// One seat's programs and where they run.
struct SeatPrograms {
    context: Arc<ExecContext>,
    tools: Vec<ExecTool>,
}

/// The source of every turn's executable tools, or `None` when no seat
/// declares any. `action_dir` gives each seat's working directory, and
/// `package_root` is exported to programs as `THM_PACKAGE_ROOT`.
pub(crate) fn belt(
    hive: &Hive,
    package_root: &Path,
    action_dir: impl Fn(&SeatPlan) -> std::path::PathBuf,
) -> Option<HostTools> {
    let by_seat: BTreeMap<String, SeatPrograms> = hive
        .seats
        .iter()
        .filter(|plan| !plan.seat.integration.programs.is_empty())
        .map(|plan| {
            let context = ExecContext {
                package_root: package_root.to_path_buf(),
                workspace: Some(action_dir(plan)),
                hive: hive.lowered.id.clone(),
                seat: plan.seat.id.clone(),
                features: BTreeMap::new(),
            };
            let programs = SeatPrograms {
                context: Arc::new(context),
                tools: plan.seat.integration.programs.clone(),
            };
            (plan.seat.id.clone(), programs)
        })
        .collect();
    if by_seat.is_empty() {
        return None;
    }
    let secrets: Arc<dyn Secrets> = Arc::new(EnvSecrets);
    Some(Arc::new(move |turn| {
        let mut belt = HostTurnTools::advertised(Vec::new());
        if let Some(seat) = by_seat.get(turn.agent_id()) {
            for tool in &seat.tools {
                belt.visible.insert(tool.id.clone());
                belt.tools.push(Box::new(ProgramTool::new(
                    tool.clone(),
                    seat.context.clone(),
                    secrets.clone(),
                )));
            }
        }
        belt
    }))
}

#[cfg(test)]
mod test;
