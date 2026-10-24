//! A sink that forwards every event to several sinks.
use super::{LoopEvent, TraceSink};
use crate::Result;

/// Records each event in every inner sink, in order.
///
/// Typical use is `events.jsonl` for the visualizer plus a [`super::TracingTrace`]
/// for an external exporter. Each inner sink stamps its own `seq` and
/// `elapsed_ms`.
#[derive(Default)]
pub struct FanoutTrace {
    sinks: Vec<Box<dyn TraceSink>>,
}

impl FanoutTrace {
    /// Starts with no sinks.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a sink; events reach sinks in the order they were added.
    #[must_use]
    pub fn with(mut self, sink: impl TraceSink + 'static) -> Self {
        self.sinks.push(Box::new(sink));
        self
    }
}

impl TraceSink for FanoutTrace {
    /// Records the event in every sink.
    ///
    /// # Errors
    ///
    /// Returns the first sink's error and stops there, so a failed primary
    /// sink is never silently skipped.
    fn record(&mut self, event: LoopEvent) -> Result<()> {
        for sink in &mut self.sinks {
            sink.record(event.clone())?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for FanoutTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FanoutTrace")
            .field("sinks", &self.sinks.len())
            .finish()
    }
}
