//! Langfuse export for the loop trace, through the `OpenHuman` client.
//!
//! [`LangfuseTrace`] is a [`tinyrsi_trace::TraceSink`]: plug it into a loop
//! (alone, or beside `events.jsonl` with a `FanoutTrace`) and each run appears
//! in Langfuse as one trace, with a span per epoch and an event per stage.
//! The transport is the `OpenHuman` embed's own `LangfuseClient`
//! (`openhuman_embed::observe::langfuse`), configured from the same
//! `LANGFUSE_*` variables `OpenHuman` agents use, so loops and agents land in
//! the same project with the same credentials.
//!
//! # Example
//!
//! ```
//! use tinyrsi_langfuse::{LangfuseClient, LangfuseTrace};
//! use tinyrsi_trace::{LoopEvent, MetricDirection, RunGoal, TraceSink};
//!
//! let client = LangfuseClient::direct("https://langfuse.example", "pk", "sk").unwrap();
//! let mut trace = LangfuseTrace::new(client)?;
//! trace.record(LoopEvent::RunStarted {
//!     run_id: "run-1".into(),
//!     subject: "solver".into(),
//!     goal: RunGoal { metric: "error".into(), direction: MetricDirection::Minimize, target: None },
//! })?;
//! assert_eq!(trace.pending()[0]["type"], "trace-create");
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
pub use sink::{LangfuseTrace, MAX_BATCH};
