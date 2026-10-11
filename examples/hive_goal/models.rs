//! The two offline model ports: the hive's agents and the proposer.
//!
//! [`ContextFollower`] stands in for the model behind the hive's seat. It
//! follows the shared context it is shown in its system prompt: only when that
//! context carries [`hive::HELPFUL`] does it work the task out; otherwise it
//! replies `UNKNOWN`, as the founding context tells it to. Whether the hive
//! succeeds therefore depends on the context document, which is the
//! thing the run improves, and the private checkers can measure it.
//!
//! [`ScriptedProposer`] stands in for the proposal model. It reads the public
//! failure dossier out of the generation request it receives (exactly what a
//! real model is shown) to ground its proposals, and replays a fixed script:
//! four bad ideas on its first call, the real repair afterwards. Two of them
//! tell the agents to call a calculator tool the hive does not have; the
//! agents then crash on every run, which the host penalizes.
use crate::hive;
use std::sync::atomic::{AtomicUsize, Ordering};
use tinyrsi_host::{
    ModelError, ModelFuture, ModelPort, ModelReply, ModelRequest, Operation, Patch, PatchClass,
    Prediction, Proposal,
};

/// Fills in a reply's usage the way a provider would (about four bytes per
/// token) and holds it to the request's reservation, as a real port must.
fn metered(request: &ModelRequest, content: String) -> Result<ModelReply, ModelError> {
    let tokens = |bytes: usize| u64::try_from(bytes.div_ceil(4)).unwrap_or(u64::MAX);
    let reply = ModelReply {
        input_tokens: tokens(request.system().len() + request.prompt().len()),
        output_tokens: tokens(content.len()).min(request.max_output_tokens()),
        content: content.into_bytes(),
    };
    request.check(&reply)?;
    Ok(reply)
}

/// The hive's agents: they do what the shared context says.
#[derive(Debug)]
pub(crate) struct ContextFollower;

/// Context that sends the agents after a tool the hive does not have.
const CRASHING: &str = "calculator tool";

impl ModelPort for ContextFollower {
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        if request.system().contains(CRASHING) {
            return Box::pin(async { Err(ModelError::CallFailed) });
        }
        let answer = if request.system().contains(hive::HELPFUL.trim_end()) {
            solve(task(request.prompt()))
        } else {
            None
        };
        let reply = metered(&request, answer.unwrap_or_else(|| "UNKNOWN".into()));
        Box::pin(async move { reply })
    }
}

/// The task text, without the coordinator messages the host appends.
fn task(prompt: &str) -> &str {
    prompt
        .split("\nVisible task messages:")
        .next()
        .unwrap_or(prompt)
}

/// Works out `Compute <a> <op> <b>`, replying in JSON when the task asks.
fn solve(task: &str) -> Option<String> {
    let mut words = task.split_once("Compute ")?.1.split_whitespace();
    let a: i64 = words.next()?.parse().ok()?;
    let op = words.next()?;
    let b: i64 = words.next()?.trim_end_matches(['.', ',']).parse().ok()?;
    let value = match op {
        "+" => a.checked_add(b)?,
        "-" => a.checked_sub(b)?,
        "*" => a.checked_mul(b)?,
        _ => return None,
    };
    Some(if task.contains("JSON") {
        serde_json::json!({ "answer": value }).to_string()
    } else {
        value.to_string()
    })
}

/// The proposal model: a fixed script grounded in the public dossier.
#[derive(Debug, Default)]
pub(crate) struct ScriptedProposer {
    calls: AtomicUsize,
}

impl ModelPort for ScriptedProposer {
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        let first = self.calls.fetch_add(1, Ordering::SeqCst) == 0;
        let reply = public_dossier(request.prompt())
            .ok_or(ModelError::CallFailed)
            .and_then(|(refs, nodes)| {
                let items = if first {
                    vec![
                        prompt_edit(&refs, &nodes),
                        tool_edit(
                            "Hand the arithmetic to a calculator tool",
                            "Always call the calculator tool before answering.\n",
                            &refs,
                            &nodes,
                        ),
                        tool_edit(
                            "Insist on the calculator tool",
                            "Never answer without the calculator tool.\n",
                            &refs,
                            &nodes,
                        ),
                        context_edit(
                            "Ask the agents for shorter answers",
                            "Short, polite replies may read as more confident.",
                            "Keep every reply short and polite.\n",
                            &refs,
                            &nodes,
                        ),
                    ]
                } else {
                    vec![context_edit(
                        "Replace the guessing instruction with a working method",
                        "The probe failed because the shared context tells every agent to guess \
                         and reply UNKNOWN; telling them to work the arithmetic out exactly and \
                         reply with only the result should make the checkers pass.",
                        hive::HELPFUL,
                        &refs,
                        &nodes,
                    )]
                };
                let items = items
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| ModelError::CallFailed)?;
                metered(&request, serde_json::json!({ "items": items }).to_string())
            });
        Box::pin(async move { reply })
    }
}

/// Transcript references and failed node ids from the public dossier inside
/// a generation request (`[{role, content: <request input JSON>}]`, whose
/// second seed carries the dossier JSON).
fn public_dossier(prompt: &str) -> Option<(Vec<u64>, Vec<String>)> {
    let messages: serde_json::Value = serde_json::from_str(prompt).ok()?;
    let input: serde_json::Value =
        serde_json::from_str(messages.get(0)?.get("content")?.as_str()?).ok()?;
    let dossier: serde_json::Value =
        serde_json::from_str(input.pointer("/seeds/1/text")?.as_str()?).ok()?;
    let refs = serde_json::from_value(dossier.get("transcript_sequences")?.clone()).ok()?;
    let nodes = serde_json::from_value(dossier.get("failed_nodes")?.clone()).ok()?;
    Some((refs, nodes))
}

/// A proposal with everything but its patch, title and mechanism fixed.
fn proposal(
    title: &str,
    mechanism: &str,
    patch: Patch,
    refs: &[u64],
    nodes: &[String],
) -> Proposal {
    Proposal {
        title: title.into(),
        patch,
        mechanism: mechanism.into(),
        prediction: Prediction {
            metric: "normalized_h".into(),
            minimum_gain: 0.1,
            nodes: nodes.to_vec(),
        },
        transcript_refs: refs.to_vec(),
        risk: "Agents could over-trust the new instruction.".into(),
        effort: "One edit.".into(),
    }
}

/// Replaces the shared context item with `after`.
fn context_edit(
    title: &str,
    mechanism: &str,
    after: &str,
    refs: &[u64],
    nodes: &[String],
) -> Proposal {
    let patch = Patch {
        class: PatchClass::Context,
        operations: vec![Operation::Context {
            path: hive::CONTEXT.into(),
            item: hive::ITEM.into(),
            before: Some(hive::UNHELPFUL.into()),
            after: Some(after.into()),
        }],
    };
    proposal(title, mechanism, patch, refs, nodes)
}

/// Adds a `tools` item to the shared context. The agents crash on any
/// context naming the calculator tool, so the host rejects the first such
/// edit `HARMFUL`; any other text for the same item has the same patch shape
/// and is cooled without evaluation.
fn tool_edit(title: &str, after: &str, refs: &[u64], nodes: &[String]) -> Proposal {
    let patch = Patch {
        class: PatchClass::Context,
        operations: vec![Operation::Context {
            path: hive::CONTEXT.into(),
            item: "tools".into(),
            before: None,
            after: Some(after.into()),
        }],
    };
    proposal(
        title,
        "A dedicated tool should compute exactly where the agents now guess.",
        patch,
        refs,
        nodes,
    )
}

/// Edits the seat prompt: a different patch class than the epoch allows.
fn prompt_edit(refs: &[u64], nodes: &[String]) -> Proposal {
    let patch = Patch {
        class: PatchClass::Prompt,
        operations: vec![Operation::Prompt {
            path: hive::SEAT_PROMPT.into(),
            before: hive::SEAT_PROMPT_BODY.into(),
            after: "Follow the shared context and double-check every answer.\n".into(),
        }],
    };
    proposal(
        "Tell the solver to double-check",
        "A seat-level reminder to check answers may catch guesses.",
        patch,
        refs,
        nodes,
    )
}
