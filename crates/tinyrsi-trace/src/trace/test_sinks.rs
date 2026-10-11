//! Behavior of the `tracing` mirror and the fan-out sink.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::*;
use std::sync::{Arc, Mutex};
use tracing::{
    Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id},
};
use tracing_subscriber::{Layer, layer::Context, prelude::*, registry::LookupSpan};

/// What the capturing layer saw, in order.
#[derive(Debug, Clone, PartialEq)]
enum Seen {
    Open {
        name: String,
        parent: Option<String>,
    },
    Event {
        kind: String,
        seq: u64,
        parent: Option<String>,
        event: String,
    },
    Close {
        name: String,
    },
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<Seen>>>);

#[derive(Default)]
struct Fields {
    kind: String,
    seq: u64,
    event: String,
}

impl Visit for Fields {
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "seq" {
            self.seq = value;
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "kind" {
            value.clone_into(&mut self.kind);
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "event" {
            self.event = format!("{value:?}");
        }
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Capture {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).unwrap();
        let parent = span.parent().map(|p| p.name().to_owned());
        assert_eq!(attrs.metadata().target(), TARGET);
        self.0.lock().unwrap().push(Seen::Open {
            name: span.name().to_owned(),
            parent,
        });
    }
    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let parent = ctx.event_span(event).map(|s| s.name().to_owned());
        self.0.lock().unwrap().push(Seen::Event {
            kind: fields.kind,
            seq: fields.seq,
            parent,
            event: fields.event,
        });
    }
    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let name = ctx.span(&id).unwrap().name().to_owned();
        self.0.lock().unwrap().push(Seen::Close { name });
    }
}

fn goal() -> RunGoal {
    RunGoal {
        metric: "rmse".into(),
        direction: MetricDirection::Minimize,
        target: Some(0.1),
    }
}

fn run() -> Vec<LoopEvent> {
    vec![
        LoopEvent::RunStarted {
            run_id: "r1".into(),
            subject: "fit".into(),
            goal: goal(),
        },
        LoopEvent::EpochStarted {
            epoch: 1,
            incumbent: "a".into(),
        },
        LoopEvent::EpochFinished {
            epoch: 1,
            incumbent: "a".into(),
            incumbent_metric: Some(0.5),
        },
        LoopEvent::RunFinished {
            reason: StopReason::MaxEpochs,
            incumbent: "a".into(),
            incumbent_metric: Some(0.5),
        },
    ]
}

fn mirrored(events: Vec<LoopEvent>) -> Vec<Seen> {
    let capture = Capture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone());
    tracing::subscriber::with_default(subscriber, || {
        let mut trace = TracingTrace::new(ManualClock::new());
        for event in events {
            trace.record(event).unwrap();
        }
    });
    capture.0.lock().unwrap().clone()
}

fn event(kind: &str, seq: u64, parent: Option<&str>) -> (String, u64, Option<String>) {
    (kind.into(), seq, parent.map(Into::into))
}

#[test]
fn nests_epoch_events_inside_the_run_span_and_closes_both() {
    let seen = mirrored(run());
    let opens: Vec<_> = seen
        .iter()
        .filter_map(|s| match s {
            Seen::Open { name, parent } => Some((name.clone(), parent.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        opens,
        vec![
            ("rsi.run".to_owned(), None),
            ("rsi.epoch".to_owned(), Some("rsi.run".to_owned()))
        ]
    );
    let events: Vec<_> = seen
        .iter()
        .filter_map(|s| match s {
            Seen::Event {
                kind, seq, parent, ..
            } => Some((kind.clone(), *seq, parent.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        events,
        vec![
            event("run_started", 0, Some("rsi.run")),
            event("epoch_started", 1, Some("rsi.epoch")),
            event("epoch_finished", 2, Some("rsi.epoch")),
            event("run_finished", 3, Some("rsi.run")),
        ]
    );
    let closes: Vec<_> = seen
        .iter()
        .filter_map(|s| match s {
            Seen::Close { name } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(closes, vec!["rsi.epoch", "rsi.run"]);
}

#[test]
fn carries_the_wire_json_of_each_event() {
    let seen = mirrored(run());
    let event = seen
        .iter()
        .find_map(|s| match s {
            Seen::Event { event, .. } => Some(event.clone()),
            _ => None,
        })
        .unwrap();
    let decoded: LoopEvent = serde_json::from_str(&event).unwrap();
    assert_eq!(decoded, run()[0]);
}

#[test]
fn events_outside_a_run_have_no_parent() {
    let seen = mirrored(vec![LoopEvent::EpochFinished {
        epoch: 4,
        incumbent: "z".into(),
        incumbent_metric: None,
    }]);
    assert!(matches!(&seen[..], [Seen::Event { parent: None, .. }]));
}

#[test]
fn a_new_run_drops_an_unfinished_epoch() {
    let mut events = run();
    events.truncate(2);
    events.push(run()[0].clone());
    let seen = mirrored(events);
    let last = seen
        .iter()
        .rev()
        .find_map(|s| match s {
            Seen::Event { parent, .. } => Some(parent.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(last.as_deref(), Some("rsi.run"));
    assert!(format!("{:?}", TracingTrace::new(ManualClock::new())).contains("run_open: false"));
}

#[test]
fn every_kind_matches_its_wire_tag() {
    let lines = include_str!("example.jsonl");
    for line in lines.lines() {
        let record: TraceRecord = serde_json::from_str(line).unwrap();
        let wire: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(record.event.kind(), wire["event"]["kind"]);
    }
}

/// A sink that always fails, to show the fan-out stops on the first error.
struct Failing;

impl TraceSink for Failing {
    fn record(&mut self, _: LoopEvent) -> Result<()> {
        Err(Error::Io)
    }
}

#[test]
fn fans_one_event_out_to_every_sink_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut fanout = FanoutTrace::new()
        .with(JsonlTrace::create(&path, ManualClock::new()).unwrap())
        .with(NullTrace);
    for event in run() {
        fanout.record(event).unwrap();
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 4);
    assert_eq!(format!("{fanout:?}"), "FanoutTrace { sinks: 2 }");
}

#[test]
fn a_failing_sink_stops_the_fan_out() {
    let mut fanout = FanoutTrace::new().with(Failing).with(NullTrace);
    assert_eq!(fanout.record(run()[0].clone()), Err(Error::Io));
    assert!(FanoutTrace::new().record(run()[0].clone()).is_ok());
}
