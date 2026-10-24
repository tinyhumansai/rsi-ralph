//! Maps loop events onto `OpenHuman` trace spans.
//!
//! One run is one trace whose root span is the run (`tinyrsi.run`). Each epoch
//! is a child span named `epoch <n>` that stays open until `epoch_finished`.
//! Every stage event is a zero-length child span named after its kind, inside
//! the open epoch or directly under the run between epochs, carrying the
//! event's wire JSON as input. `run_finished` closes the run with the outcome
//! as output (including the final incumbent metric) and hands the finished
//! spans over for export.
use openhuman_embed::observe::langfuse::{SpanKind, SpanStatus, TraceSpan};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use rsi_ralph_trace::{LoopEvent, StopReason};

/// Milliseconds since the Unix epoch, or zero for a clock set before it.
pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The spans of the run in progress.
#[derive(Debug, Default)]
pub(super) struct SpanBuilder {
    spans: Vec<TraceSpan>,
    epoch: Option<usize>,
    seq: u64,
}

impl SpanBuilder {
    fn run_id(&self) -> Option<String> {
        self.spans.first().map(|root| root.trace_id.clone())
    }

    fn span(&self, id: String, parent: String, name: String, kind: SpanKind, at: u64) -> TraceSpan {
        TraceSpan {
            trace_id: self.run_id().unwrap_or_default(),
            span_id: id,
            parent_span_id: Some(parent),
            name,
            kind,
            start_unix_ms: at,
            end_unix_ms: None,
            status: SpanStatus::Unset,
            attributes: BTreeMap::new(),
            input: None,
            output: None,
        }
    }

    /// A finished zero-length span for one stage, under the open epoch.
    fn stage(&mut self, run: &str, event: &LoopEvent, wire: Value, at: u64) {
        let parent = self
            .epoch
            .map_or_else(|| run.to_owned(), |i| self.spans[i].span_id.clone());
        let mut span = self.span(
            format!("{run}:{}", self.seq),
            parent,
            event.kind().to_owned(),
            SpanKind::Iteration,
            at,
        );
        self.seq += 1;
        span.end_unix_ms = Some(at);
        span.status = SpanStatus::Ok;
        span.input = Some(wire);
        self.spans.push(span);
    }

    /// Records `event` at `at`; returns the run's finished spans when the
    /// event ends a run. Events outside a run have no trace and are dropped.
    pub(super) fn push(&mut self, event: &LoopEvent, at: u64) -> Option<Vec<TraceSpan>> {
        let wire = serde_json::to_value(event).unwrap_or(Value::Null);
        if let LoopEvent::RunStarted {
            run_id,
            subject,
            goal,
        } = event
        {
            *self = Self::default();
            let mut attributes = BTreeMap::new();
            attributes.insert("run.type".to_owned(), json!("rsi_loop"));
            self.spans.push(TraceSpan {
                trace_id: run_id.clone(),
                span_id: run_id.clone(),
                parent_span_id: None,
                name: "tinyrsi.run".into(),
                kind: SpanKind::Turn,
                start_unix_ms: at,
                end_unix_ms: None,
                status: SpanStatus::Unset,
                attributes,
                input: Some(json!({"subject": subject, "goal": goal})),
                output: None,
            });
            return None;
        }
        let run = self.run_id()?;
        match event {
            LoopEvent::EpochStarted { epoch, incumbent } => {
                let mut span = self.span(
                    format!("{run}:epoch:{epoch}"),
                    run.clone(),
                    format!("epoch {epoch}"),
                    SpanKind::Iteration,
                    at,
                );
                span.input = Some(json!({"incumbent": incumbent}));
                self.spans.push(span);
                self.epoch = Some(self.spans.len() - 1);
                None
            }
            LoopEvent::EpochFinished { .. } => {
                self.stage(&run, event, wire.clone(), at);
                if let Some(i) = self.epoch.take() {
                    let span = &mut self.spans[i];
                    span.end_unix_ms = Some(at);
                    span.status = SpanStatus::Ok;
                    span.output = Some(wire);
                }
                None
            }
            LoopEvent::RunFinished { reason, .. } => {
                self.stage(&run, event, wire.clone(), at);
                // A failed run (an error, or a provider that refused every
                // call) interrupts whatever is still open, so an unfinished
                // epoch reads as failed rather than completed.
                let status = if matches!(reason, StopReason::Error | StopReason::ProviderRefused) {
                    SpanStatus::Error
                } else {
                    SpanStatus::Ok
                };
                for span in &mut self.spans {
                    if span.end_unix_ms.is_none() {
                        span.end_unix_ms = Some(at);
                        span.status = status;
                    }
                }
                let root = &mut self.spans[0];
                root.output = Some(wire);
                root.status = status;
                Some(std::mem::take(self).spans)
            }
            _ => {
                self.stage(&run, event, wire, at);
                None
            }
        }
    }
}
