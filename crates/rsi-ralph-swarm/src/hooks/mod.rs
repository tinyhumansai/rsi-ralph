//! Turn hooks: stream each seat's tool calls and per-turn usage into the
//! trace, keep the per-seat tally [`crate::SwarmOutcome`] reports, and,
//! when seats carry memory, label every turn so it is remembered.
//!
//! `OpenHuman` logs a turn to memory only when its origin says a person wrote
//! it. A seat's turn usually starts inside another seat's `hivemind_*` call
//! and inherits that call's sub-agent origin, so without a label only turns
//! the host started would be remembered. With memory on, every seat turn is
//! labelled `DirectChat`: the approval gate trusts it exactly as it trusts
//! the sub-agent default (`Cli`), so the label changes what is remembered,
//! not what a seat may do.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use openhuman_embed::agent_progress::AgentProgress;
use openhuman_embed::{AgentTurnOrigin, LastTurnUsage};
use tinyhivemind_hives::TurnDisposition;
use tinyhivemind_openhuman::{TurnHooks, TurnProgressSink, TurnScope};

use crate::SeatUsage;
use crate::event::{SwarmEvent, ToolPhase, clip};
use crate::trace::Trace;

/// How much of a tool call's arguments the trace keeps.
const DETAIL_CHARS: usize = 240;

/// The swarm's [`TurnHooks`].
#[derive(Debug)]
pub(crate) struct SwarmHooks {
    trace: Arc<Trace>,
    usage: Mutex<BTreeMap<String, SeatUsage>>,
    /// Whether seats carry memory, so every turn is labelled to be logged.
    remembered: bool,
}

impl SwarmHooks {
    /// Hooks reporting into `trace`.
    pub(crate) fn new(trace: Arc<Trace>) -> Self {
        Self {
            trace,
            usage: Mutex::new(BTreeMap::new()),
            remembered: false,
        }
    }

    /// Label every turn so `OpenHuman` logs it to the seats' memory.
    #[must_use]
    pub(crate) fn remembered(mut self, remembered: bool) -> Self {
        self.remembered = remembered;
        self
    }

    /// Every seat's turns and spend so far.
    pub(crate) fn usage(&self) -> BTreeMap<String, SeatUsage> {
        self.usage
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Adds one finished turn to `seat`'s tally and reports it.
    fn record_turn(&self, seat: &str, usage: Option<&LastTurnUsage>) {
        let (input_tokens, output_tokens, cost_usd) = usage.map_or((0, 0, None), |usage| {
            (usage.input_tokens, usage.output_tokens, usage.cost_usd)
        });
        {
            let mut tally = self.usage.lock().unwrap_or_else(PoisonError::into_inner);
            let entry = tally.entry(seat.to_owned()).or_default();
            entry.turns += 1;
            entry.input_tokens += input_tokens;
            entry.output_tokens += output_tokens;
            match cost_usd {
                Some(cost) => entry.cost_usd += cost,
                None => entry.cost_complete = false,
            }
        }
        self.trace.emit(SwarmEvent::AgentTurn {
            seat: seat.to_owned(),
            input_tokens,
            output_tokens,
            cost_usd,
        });
    }
}

/// The trace event for one progress update, if it is a tool call.
pub(crate) fn tool_event(seat: &str, progress: AgentProgress) -> Option<SwarmEvent> {
    match progress {
        AgentProgress::ToolCallStarted {
            tool_name,
            arguments,
            display_detail,
            ..
        } => Some(SwarmEvent::ToolCall {
            seat: seat.to_owned(),
            tool: tool_name,
            phase: ToolPhase::Started,
            detail: display_detail
                .or_else(|| (!arguments.is_null()).then(|| arguments.to_string()))
                .map(|detail| clip(&detail, DETAIL_CHARS)),
            success: None,
            output_chars: None,
        }),
        AgentProgress::ToolCallCompleted {
            tool_name,
            success,
            output_chars,
            ..
        } => Some(SwarmEvent::ToolCall {
            seat: seat.to_owned(),
            tool: tool_name,
            phase: ToolPhase::Completed,
            detail: None,
            success: Some(success),
            output_chars: Some(output_chars),
        }),
        _ => None,
    }
}

impl TurnHooks for SwarmHooks {
    fn configure(&self, _scope: &TurnScope, turn: openhuman_embed::Turn) -> openhuman_embed::Turn {
        if self.remembered {
            turn.origin(AgentTurnOrigin::DirectChat)
        } else {
            turn
        }
    }

    fn progress(&self, scope: &TurnScope) -> Option<TurnProgressSink> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<AgentProgress>(256);
        let trace = self.trace.clone();
        let seat = scope.agent_id.clone();
        tokio::spawn(async move {
            while let Some(progress) = receiver.recv().await {
                if let Some(event) = tool_event(&seat, progress) {
                    trace.emit(event);
                }
            }
        });
        Some(sender)
    }

    fn after_turn(
        &self,
        scope: &TurnScope,
        usage: Option<&LastTurnUsage>,
    ) -> tinyhivemind_openhuman::Result<TurnDisposition> {
        self.record_turn(&scope.agent_id, usage);
        Ok(TurnDisposition::Completed)
    }
}

#[cfg(test)]
mod test;
