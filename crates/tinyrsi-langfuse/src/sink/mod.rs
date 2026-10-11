//! A [`TraceSink`] that exports loop events to Langfuse.
//!
//! Events are mapped as they arrive (see `batch.rs`) and buffered.
//! `run_finished` flushes the buffer through the `OpenHuman` `LangfuseClient` in
//! chunks of at most [`MAX_BATCH`] events, the ingestion API's limit. Export
//! is best-effort: the trace is observability, not evidence, so a failed
//! export never fails the loop. It is counted in [`LangfuseTrace::failures`],
//! the events stay buffered, and an explicit [`LangfuseTrace::flush`] retries
//! and reports the error.
mod batch;
mod time;

use crate::{Error, Result};
use batch::BatchBuilder;
use openhuman_embed::observe::langfuse::LangfuseClient;
use serde_json::{Value, json};
use tinyrsi_trace::{LoopEvent, TraceSink};

/// Langfuse's limit on events per ingestion request.
pub const MAX_BATCH: usize = 500;

/// Exports loop events to Langfuse as one trace per run.
pub struct LangfuseTrace {
    client: LangfuseClient,
    runtime: tokio::runtime::Runtime,
    builder: BatchBuilder,
    pending: Vec<Value>,
    failures: u64,
}

impl LangfuseTrace {
    /// Exports through `client`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidConfig`] when the single-threaded runtime that
    /// drives the client's HTTP transport cannot be started.
    pub fn new(client: LangfuseClient) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::InvalidConfig)?;
        Ok(Self {
            client,
            runtime,
            builder: BatchBuilder::default(),
            pending: vec![],
            failures: 0,
        })
    }

    /// Exports to the Langfuse project named by the environment:
    /// `LANGFUSE_BASE_URL`, `LANGFUSE_PUBLIC_KEY` and `LANGFUSE_SECRET_KEY`, or
    /// the `TinyHumans` proxy (`TINYHUMANS_LANGFUSE_PROXY_URL` with
    /// `TINYHUMANS_AUTH_TOKEN`), exactly as `OpenHuman` reads them.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidConfig`] when a required variable is missing.
    pub fn from_env() -> Result<Self> {
        Self::new(LangfuseClient::from_env().map_err(|_| Error::InvalidConfig)?)
    }

    /// Ingestion events mapped but not yet delivered.
    #[must_use]
    pub fn pending(&self) -> &[Value] {
        &self.pending
    }

    /// How many automatic flushes failed.
    #[must_use]
    pub fn failures(&self) -> u64 {
        self.failures
    }

    /// Sends every pending event, at most [`MAX_BATCH`] per request.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ExportFailed`] when Langfuse cannot be reached or
    /// rejects any event. Events from requests that did not succeed stay
    /// pending, so a later flush retries them.
    pub fn flush(&mut self) -> Result<()> {
        while !self.pending.is_empty() {
            let take = self.pending.len().min(MAX_BATCH);
            let batch = json!({"batch": &self.pending[..take]});
            self.runtime
                .block_on(self.client.send_batch(batch))
                .map_err(|_| Error::ExportFailed)?;
            self.pending.drain(..take);
        }
        Ok(())
    }
}

impl TraceSink for LangfuseTrace {
    /// Maps and buffers the event; `run_finished` flushes. Never fails: a
    /// failed flush is counted and its events kept for a later retry.
    fn record(&mut self, event: LoopEvent) -> tinyrsi_trace::Result<()> {
        self.pending
            .extend(self.builder.push(&event, time::now_ms()));
        if matches!(event, LoopEvent::RunFinished { .. }) && self.flush().is_err() {
            self.failures += 1;
        }
        Ok(())
    }
}

impl std::fmt::Debug for LangfuseTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LangfuseTrace")
            .field("endpoint", &self.client.endpoint())
            .field("pending", &self.pending.len())
            .field("failures", &self.failures)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test;
