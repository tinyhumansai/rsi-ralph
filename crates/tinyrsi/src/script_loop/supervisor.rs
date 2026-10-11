//! The language-model supervisor: one forced `propose_script` tool call per
//! proposal, over the crate's bounded [`ModelPort`].
use super::{
    LedgerCandidate, ScriptLedger, ScriptProposal, ScriptSubject, SupervisorLimits,
    execute::Sources,
};
use crate::{Error, ModelPort, ModelRequest, Result};
use serde_json::{Value, json};
use std::{
    fmt::Write as _,
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread::{self, Thread},
};

/// Name of the forced native tool.
pub(super) const TOOL: &str = "propose_script";

const SYSTEM: &str = "You are the supervisor of an automated improvement loop for a script. \
Each turn you propose ONE concrete change that should improve the script's metric. \
Call the propose_script tool exactly once. Every file you list replaces that file entirely, \
so return complete file contents, never diffs or excerpts. Only edit the listed editable paths. \
The script must keep printing its metric line on stdout and must stay deterministic for a given \
TINYRSI_SEED. Learn from rejected proposals: do not repeat them.";

/// Proposes script changes through a [`ModelPort`].
///
/// The port's futures are driven on the calling thread, so the port must not
/// require a specific async runtime; [`crate::OpenRouterModel`] does not.
#[derive(Clone)]
pub struct Supervisor {
    port: Arc<dyn ModelPort>,
    limits: SupervisorLimits,
}

impl std::fmt::Debug for Supervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Supervisor")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

/// One supervisor call: measured tokens and the decoded proposal.
#[derive(Debug)]
pub(super) struct SupervisorTurn {
    pub tokens: u64,
    pub proposal: Result<ScriptProposal>,
}

impl Supervisor {
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

    /// Asks for one proposal.
    ///
    /// A reply that is not exactly one [`ScriptProposal`] object is returned as
    /// `Err(SupervisorMalformed)` inside the turn, so the loop can record it and
    /// continue; the outer error is a failed or over-budget model call.
    pub(super) fn propose(
        &self,
        subject: &ScriptSubject,
        prompt: String,
        seed: u64,
    ) -> Result<SupervisorTurn> {
        let request = ModelRequest::new(
            SYSTEM.into(),
            prompt,
            seed,
            self.limits.call_tokens,
            self.limits.output_tokens,
            self.limits.timeout_ms,
        )?
        .with_tool(TOOL.into(), schema(subject))?;
        let reply = block_on(self.port.generate(request.clone()))?;
        request.check(&reply)?;
        Ok(SupervisorTurn {
            tokens: reply.input_tokens.saturating_add(reply.output_tokens),
            proposal: serde_json::from_slice(&reply.content).map_err(|_| Error::SupervisorMalformed),
        })
    }
}

/// JSON schema of the tool arguments, with paths restricted to `editable`.
pub(super) fn schema(subject: &ScriptSubject) -> Value {
    let paths: Vec<String> = subject
        .editable
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

/// Builds the supervisor prompt from the subject, ledger and incumbent.
pub(super) fn prompt(
    subject: &ScriptSubject,
    ledger: &ScriptLedger,
    sources: &Sources,
    rejected_context: usize,
) -> String {
    let metric = &subject.metric;
    let mut text = String::new();
    let _ = writeln!(text, "# Goal\n{}\n", subject.brief);
    let _ = writeln!(
        text,
        "Metric `{}` ({:?}); target: {}. The script prints `METRIC {}=<value>` on stdout.",
        metric.name,
        metric.direction,
        metric
            .target
            .map_or_else(|| "none".to_owned(), |t| t.to_string()),
        metric.name
    );
    let _ = writeln!(
        text,
        "Command: {} {} (run once per seed {:?} with TINYRSI_SEED set; cleared environment).\n",
        subject.command.program.display(),
        subject.command.args.join(" "),
        subject.seeds
    );
    let _ = writeln!(text, "# Metric history (incumbent per epoch)");
    for entry in &ledger.incumbents {
        let _ = writeln!(
            text,
            "- epoch {}: {} = {}",
            entry.epoch, entry.id, entry.metric
        );
    }
    let _ = writeln!(text, "\n# Recently rejected proposals");
    let rejected: Vec<&LedgerCandidate> = ledger
        .candidates
        .iter()
        .rev()
        .filter(|c| !c.promoted && c.proposal_id.is_some())
        .filter(|c| c.decision != super::CandidateDecision::Accept)
        .take(rejected_context)
        .collect();
    if rejected.is_empty() {
        let _ = writeln!(text, "None yet.");
    }
    for c in rejected {
        let stderr = c
            .runs
            .iter()
            .find(|r| r.error.is_some())
            .or(c.runs.first())
            .map_or("", |r| r.stderr_tail.as_str());
        let errors: Vec<&str> = c.runs.iter().filter_map(|r| r.error.as_deref()).collect();
        let _ = writeln!(
            text,
            "- {:?}: decision {:?}, mean {:?}, errors {:?}, note {:?}\n  rationale: {}\n  hypothesis: {}\n  stderr tail: {}",
            c.proposal_id.as_deref().unwrap_or_default(),
            c.decision,
            c.mean,
            errors,
            c.note.as_deref().unwrap_or_default(),
            c.rationale,
            c.hypothesis,
            stderr.trim()
        );
    }
    let _ = writeln!(text, "\n# Incumbent editable sources");
    for (path, source) in sources {
        let _ = writeln!(
            text,
            "\n## {}\n```\n{}\n```",
            path.display(),
            source.trim_end()
        );
    }
    text
}

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Drives a future to completion on the current thread.
pub(super) fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}
