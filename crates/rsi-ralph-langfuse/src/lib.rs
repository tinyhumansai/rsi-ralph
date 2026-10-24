//! Langfuse export for the loop trace, through the `OpenHuman` client.
//!
//! [`LangfuseTrace`] is a [`rsi_ralph_trace::TraceSink`]: plug it into a loop
//! (alone, or beside `events.jsonl` with a `FanoutTrace`) and each run appears
//! in Langfuse as one trace, with a span per epoch and a span per stage.
//! It builds `OpenHuman` trace spans and sends them over OTLP, Langfuse's
//! supported trace path, through the `OpenHuman` embed's own `LangfuseClient`
//! (`openhuman_embed::observe::langfuse`). Configuration comes from the same
//! `LANGFUSE_*` variables `OpenHuman` agents use, so loops and agents land in
//! the same project with the same credentials.
//!
//! # Example
//!
//! ```
//! use rsi_ralph_langfuse::{LangfuseClient, LangfuseTrace};
//! use rsi_ralph_trace::{LoopEvent, MetricDirection, RunGoal, TraceSink};
//!
//! let client = LangfuseClient::direct("https://langfuse.example", "pk", "sk").unwrap();
//! let mut trace = LangfuseTrace::new(client)?;
//! trace.record(LoopEvent::RunStarted {
//!     run_id: "run-1".into(),
//!     subject: "solver".into(),
//!     goal: RunGoal { metric: "error".into(), direction: MetricDirection::Minimize, target: None },
//! })?;
//! // Spans are exported when the run finishes; nothing is pending yet.
//! assert!(trace.pending().is_empty());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # What this crate does not hold
//!
//! No HTTP client of its own and no `tracing` subscriber. It is a separate
//! crate because `openhuman-embed` is a large dependency, and hosts that only
//! need `events.jsonl` should not compile it.

mod error;
mod sink;

pub use error::{Error, Result};
pub use openhuman_embed::observe::langfuse::{LangfuseAuth, LangfuseClient};
pub use sink::LangfuseTrace;
