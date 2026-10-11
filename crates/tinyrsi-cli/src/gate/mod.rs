//! Gates a candidate must pass before it is scored.
//!
//! The surface check always runs first: every changed path must sit under an
//! `editable` prefix and under no `frozen` prefix, and with
//! `require_test_change` at least one changed path must mention `test`. Then
//! the manifest's gate commands run in order under the step contract, and the
//! first failure stops the gate.

use std::{path::Path, time::Duration};

use crate::{
    Result,
    manifest::{GateStep, StepCwd, SubjectSpec},
    step::{self, StepContext, StepOutcome, StepRun},
    subject::Checkout,
};

/// The surface check as a step outcome, so it is stored like any gate.
#[must_use]
pub fn surface(spec: &SubjectSpec, changed: &[String]) -> StepOutcome {
    let under = |path: &str, prefix: &str| {
        let prefix = prefix.trim_start_matches("./");
        path == prefix.trim_end_matches('/')
            || path.starts_with(prefix)
                && (prefix.ends_with('/') || path[prefix.len()..].starts_with('/'))
    };
    let outside = changed.iter().find(|path| {
        !spec.editable.iter().any(|p| under(path, p)) || spec.frozen.iter().any(|p| under(path, p))
    });
    let reason = if let Some(path) = outside {
        Some(format!("{path} is outside the editable surface"))
    } else if spec.require_test_change && !changed.iter().any(|p| p.contains("test")) {
        Some("no test file changed".to_owned())
    } else {
        None
    };
    StepOutcome {
        id: "gate.surface".into(),
        ok: reason.is_none(),
        reason,
        duration_ms: 0,
        stdout: String::new(),
    }
}

/// Run the gate commands for one candidate, stopping at the first failure.
///
/// Every outcome, passing or not, is returned in order.
///
/// # Errors
/// Returns an error only when a command cannot be started at all.
pub fn run_gates(
    ctx: &StepContext,
    gates: &[GateStep],
    candidate: &Checkout,
    dir: &Path,
) -> Result<Vec<StepOutcome>> {
    let mut outcomes = Vec::new();
    for (i, gate) in gates.iter().enumerate() {
        let cwd = match gate.cwd {
            StepCwd::Loop => ctx.loop_dir.clone(),
            StepCwd::Candidate => candidate.path.clone(),
        };
        let outcome = step::run(
            ctx,
            &StepRun {
                id: format!("gate.{i}"),
                command: gate.run.clone(),
                cwd,
                candidate: candidate.path.clone(),
                label: candidate.label.clone(),
                seed: 0,
                dir: dir.join(format!("gate.{i}")),
                timeout: Duration::from_secs(gate.timeout_secs),
            },
        )?;
        let ok = outcome.ok;
        outcomes.push(outcome);
        if !ok {
            break;
        }
    }
    Ok(outcomes)
}

#[cfg(test)]
mod test;
