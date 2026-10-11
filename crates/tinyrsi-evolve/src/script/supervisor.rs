//! The language-model supervisor: one forced `propose_script` tool call per
//! proposal, over a bounded [`ModelPort`].
use super::{ScriptBrief, ScriptProposal, SupervisorLimits};
use crate::{Error, Result};
use crate::{EvolveContext, Evolver, Proposed};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use tinyrsi_model::{ModelPort, ModelRequest, block_on};
use tinyrsi_types::Goal;

/// Name of the forced native tool.
pub(crate) const TOOL: &str = "propose_script";

const SYSTEM: &str = "You are the supervisor of an automated improvement loop for a script. \
Each turn you propose ONE concrete change that should improve the script's metric. \
Call the propose_script tool exactly once. Every file you list replaces that file entirely, \
so return complete file contents, never diffs or excerpts. Only edit the listed editable paths. \
The script must keep printing its metric line on stdout and must stay deterministic for a given \
TINYRSI_SEED. Learn from rejected proposals: do not repeat them.";

/// Proposes script changes through a [`ModelPort`].
///
/// The port's futures are driven on the calling thread, so the port must not
/// require a specific async runtime; `tinyrsi_model::OpenRouterModel` does
/// not.
#[derive(Clone)]
pub struct ScriptSupervisor {
    port: Arc<dyn ModelPort>,
    limits: SupervisorLimits,
}

impl std::fmt::Debug for ScriptSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptSupervisor")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl ScriptSupervisor {
    /// Wraps a model port with per-call limits.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidScriptSubject`] for a zero limit, an output cap
    /// not below the call cap, or a timeout above 300 000 ms.
    pub fn new(port: Arc<dyn ModelPort>, limits: SupervisorLimits) -> Result<Self> {
        if limits.call_tokens == 0
            || limits.output_tokens == 0
            || limits.output_tokens >= limits.call_tokens
            || limits.timeout_ms == 0
            || limits.timeout_ms > 300_000
        {
            return Err(Error::InvalidScriptSubject);
        }
        Ok(Self { port, limits })
    }
}

impl Evolver<ScriptBrief> for ScriptSupervisor {
    type Proposal = ScriptProposal;

    /// Asks for one proposal with the brief's prompt and the context's seed.
    ///
    /// A reply that is not exactly one valid [`ScriptProposal`] is returned as
    /// a rejected proposal inside [`Proposed`], so the loop can record it and
    /// continue.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Model`] for a failed, refused or over-budget call.
    fn propose(
        &self,
        brief: &ScriptBrief,
        _: &Goal,
        context: &EvolveContext,
    ) -> Result<Proposed<ScriptProposal>> {
        let request = ModelRequest::new(
            SYSTEM.into(),
            brief.prompt.clone(),
            context.seed,
            self.limits.call_tokens,
            self.limits.output_tokens,
            self.limits.timeout_ms,
        )?
        .with_tool(TOOL.into(), schema(&brief.editable))?;
        let reply = block_on(self.port.generate(request.clone()))?;
        request.check(&reply)?;
        let proposal = serde_json::from_slice::<ScriptProposal>(&reply.content)
            .map_err(|_| Error::SupervisorMalformed)
            .and_then(|p| p.validate(&brief.editable).map(|()| p));
        Ok(Proposed {
            tokens: reply.input_tokens.saturating_add(reply.output_tokens),
            proposals: vec![proposal],
        })
    }
}

/// JSON schema of the tool arguments, with paths restricted to `editable`.
pub(crate) fn schema(editable: &[PathBuf]) -> Value {
    let paths: Vec<String> = editable
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["rationale", "hypothesis", "files"],
        "properties": {
            "rationale": {"type": "string", "description": "Why this change should improve the metric."},
            "hypothesis": {"type": "string", "description": "The expected, checkable effect on the metric."},
            "files": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["path", "content"],
                    "properties": {
                        "path": {"type": "string", "enum": paths},
                        "content": {"type": "string", "description": "Complete new file content."}
                    }
                }
            }
        }
    })
}
