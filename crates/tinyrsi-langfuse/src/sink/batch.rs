//! Maps loop events onto Langfuse ingestion events.
//!
//! One run is one trace, whose id is the run id. Each epoch is a span with the
//! deterministic id `<run>:epoch:<n>`, so a re-sent batch upserts rather than
//! duplicates. Every stage event is an `event-create` inside the open epoch
//! span (or directly under the trace between epochs), carrying the event's
//! wire JSON as input. `run_finished` updates the trace with its outcome and,
//! when there is one, records the final incumbent metric as a numeric score.
use super::time::iso8601;
use serde_json::{Value, json};
use tinyrsi_trace::LoopEvent;

/// Tracks the open run and epoch while turning events into ingestion events.
#[derive(Debug, Default)]
pub(super) struct BatchBuilder {
    run: Option<String>,
    epoch: Option<String>,
    seq: u64,
}

impl BatchBuilder {
    /// Ingestion events for `event`, stamped `now_ms`. Events outside a run
    /// have no trace to belong to and map to nothing.
    pub(super) fn push(&mut self, event: &LoopEvent, now_ms: u64) -> Vec<Value> {
        let at = iso8601(now_ms);
        if let LoopEvent::RunStarted {
            run_id,
            subject,
            goal,
        } = event
        {
            self.run = Some(run_id.clone());
            self.epoch = None;
            return vec![self.envelope(
                "trace-create",
                &at,
                &json!({
                    "id": run_id,
                    "name": "tinyrsi.run",
                    "timestamp": at,
                    "input": {"subject": subject, "goal": goal},
                    "tags": ["tinyrsi"],
                }),
            )];
        }
        let Some(run) = self.run.clone() else {
            return vec![];
        };
        let wire = serde_json::to_value(event).unwrap_or(Value::Null);
        match event {
            LoopEvent::EpochStarted { epoch, incumbent } => {
                let id = format!("{run}:epoch:{epoch}");
                self.epoch = Some(id.clone());
                vec![self.envelope(
                    "span-create",
                    &at,
                    &json!({
                        "id": id,
                        "traceId": run,
                        "name": format!("epoch {epoch}"),
                        "startTime": at,
                        "input": {"incumbent": incumbent},
                    }),
                )]
            }
            LoopEvent::EpochFinished { epoch, .. } => {
                let mut out = vec![self.stage(&run, event, &wire, &at)];
                out.push(self.envelope(
                    "span-update",
                    &at,
                    &json!({
                        "id": format!("{run}:epoch:{epoch}"),
                        "traceId": run,
                        "endTime": at,
                        "output": wire,
                    }),
                ));
                self.epoch = None;
                out
            }
            LoopEvent::RunFinished {
                incumbent_metric, ..
            } => {
                self.epoch = None;
                let mut out = vec![
                    self.stage(&run, event, &wire, &at),
                    self.envelope(
                        "trace-create",
                        &at,
                        &json!({"id": run, "timestamp": at, "output": wire}),
                    ),
                ];
                if let Some(metric) = incumbent_metric {
                    out.push(self.envelope(
                        "score-create",
                        &at,
                        &json!({
                            "id": format!("{run}:incumbent_metric"),
                            "traceId": run,
                            "name": "incumbent_metric",
                            "value": metric,
                            "dataType": "NUMERIC",
                        }),
                    ));
                }
                self.run = None;
                out
            }
            _ => vec![self.stage(&run, event, &wire, &at)],
        }
    }

    /// An `event-create` for one stage, inside the open epoch if any.
    fn stage(&mut self, run: &str, event: &LoopEvent, wire: &Value, at: &str) -> Value {
        let mut body = json!({
            "id": format!("{run}:{}", self.seq),
            "traceId": run,
            "name": event.kind(),
            "startTime": at,
            "input": wire,
        });
        if let Some(epoch) = &self.epoch {
            body["parentObservationId"] = json!(epoch);
        }
        self.envelope("event-create", at, &body)
    }

    /// Wraps a body in the ingestion envelope with a unique event id.
    fn envelope(&mut self, kind: &str, at: &str, body: &Value) -> Value {
        let id = format!(
            "{}:{}:{kind}",
            self.run.as_deref().unwrap_or("tinyrsi"),
            self.seq
        );
        self.seq += 1;
        json!({"id": id, "timestamp": at, "type": kind, "body": body})
    }
}
