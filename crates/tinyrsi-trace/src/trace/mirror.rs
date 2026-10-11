//! A sink that mirrors loop events into the `tracing` ecosystem.
//!
//! Each run becomes a `rsi.run` span and each epoch an `rsi.epoch` span inside
//! it; every event is a `tracing` event at `INFO` under the target
//! [`TARGET`], carrying `seq`, `elapsed_ms`, `kind` and the event's wire JSON
//! in `event`. Whatever subscriber the host installed (a formatter, a file
//! appender, or an OpenTelemetry/Langfuse exporter) receives the same stream
//! the visualizer reads from `events.jsonl`.
use super::{Clock, LoopEvent, Stamp, TraceSink};
use crate::{Error, Result};
use tracing::{Level, Span};

/// The `tracing` target every mirrored span and event uses.
pub const TARGET: &str = "tinyrsi::loop";

/// Mirrors loop events as `tracing` spans and events.
pub struct TracingTrace {
    stamp: Stamp,
    run: Option<Span>,
    epoch: Option<Span>,
}

impl TracingTrace {
    /// Starts with no open run; the first `run_started` opens one.
    #[must_use]
    pub fn new(clock: impl Clock + 'static) -> Self {
        Self {
            stamp: Stamp {
                seq: 0,
                clock: Box::new(clock),
            },
            run: None,
            epoch: None,
        }
    }

    /// The innermost open span, if any.
    fn current(&self) -> Option<&Span> {
        self.epoch.as_ref().or(self.run.as_ref())
    }
}

impl TraceSink for TracingTrace {
    fn record(&mut self, event: LoopEvent) -> Result<()> {
        match &event {
            LoopEvent::RunStarted {
                run_id, subject, ..
            } => {
                self.epoch = None;
                // An explicit empty parent: a run is its own trace, never a
                // child of whatever span the host happens to be inside.
                self.run = Some(tracing::span!(
                    target: TARGET,
                    parent: None,
                    Level::INFO,
                    "rsi.run",
                    run_id = %run_id,
                    subject = %subject
                ));
            }
            LoopEvent::EpochStarted { epoch, .. } => {
                let parent = self.run.as_ref().and_then(Span::id);
                self.epoch = Some(tracing::span!(
                    target: TARGET,
                    parent: parent,
                    Level::INFO,
                    "rsi.epoch",
                    epoch = epoch
                ));
            }
            _ => {}
        }
        let kind = event.kind();
        let record = self.stamp.next(event);
        let json = serde_json::to_string(&record.event).map_err(|_| Error::Io)?;
        let parent = self.current().and_then(Span::id);
        tracing::event!(
            target: TARGET,
            parent: parent,
            Level::INFO,
            seq = record.seq,
            elapsed_ms = record.elapsed_ms,
            kind = kind,
            event = %json
        );
        match kind {
            "epoch_finished" => self.epoch = None,
            "run_finished" => {
                self.epoch = None;
                self.run = None;
            }
            _ => {}
        }
        Ok(())
    }
}

impl std::fmt::Debug for TracingTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TracingTrace")
            .field("seq", &self.stamp.seq)
            .field("run_open", &self.run.is_some())
            .field("epoch_open", &self.epoch.is_some())
            .finish_non_exhaustive()
    }
}
