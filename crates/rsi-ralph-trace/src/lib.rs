//! The loop trace: what an improvement loop is doing, as it happens.
//!
//! A loop emits [`LoopEvent`]s (one per stage boundary) through a
//! [`TraceSink`]. [`JsonlTrace`] appends them to `events.jsonl`, the file the
//! `apps/rsi-viz` visualizer tails. [`TracingTrace`] mirrors them into
//! `tracing` spans and events under the [`TARGET`] target, so any subscriber
//! the host installs (including an OpenTelemetry or Langfuse exporter) sees
//! the same run. [`FanoutTrace`] sends one stream to several sinks.
//!
//! # Example
//!
//! ```
//! use rsi_ralph_trace::{LoopEvent, ManualClock, TraceSink, VecTrace};
//!
//! let mut trace = VecTrace::new(ManualClock::new());
//! trace.record(LoopEvent::EpochStarted { epoch: 1, incumbent: "abc".into() })?;
//! assert_eq!(trace.records()[0].seq, 0);
//! assert_eq!(trace.records()[0].event.kind(), "epoch_started");
//! # Ok::<(), rsi_ralph_trace::Error>(())
//! ```
//!
//! # What this crate does not hold
//!
//! No exporter and no subscriber. Installing a `tracing` subscriber is the
//! host's decision, and the trace is observability rather than evidence:
//! nothing recovers from it.

mod error;
mod trace;

pub use error::{Error, Result};
pub use trace::*;
