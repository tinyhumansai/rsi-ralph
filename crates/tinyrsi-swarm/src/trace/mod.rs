//! The one place swarm events leave: `events.jsonl` and the caller's channel.
//!
//! Every event is scrubbed by the run's [`Redactor`] first, numbered, and
//! appended as one JSON line in the envelope documented in [`crate::event`].
//! A write failure does not stop the swarm mid-epoch; the first one is kept
//! and returned by [`Trace::finish`].

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use serde_json::json;

use crate::event::{Redactor, SwarmEvent};
use crate::{Error, Result};

/// An append-only, thread-safe event sink.
#[derive(Debug)]
pub(crate) struct Trace {
    path: PathBuf,
    state: Mutex<State>,
    started: Instant,
    redactor: Redactor,
    events: Sender<SwarmEvent>,
}

#[derive(Debug)]
struct State {
    file: std::fs::File,
    next: u64,
    failure: Option<std::io::Error>,
}

impl Trace {
    /// Opens (appending to) `path`; events are also sent to `events`.
    pub(crate) fn create(
        path: &Path,
        redactor: Redactor,
        events: Sender<SwarmEvent>,
    ) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| Error::RunDir {
                path: path.to_path_buf(),
                source,
            })?;
        Ok(Self {
            path: path.to_path_buf(),
            state: Mutex::new(State {
                file,
                next: 0,
                failure: None,
            }),
            started: Instant::now(),
            redactor,
            events,
        })
    }

    /// Scrubs, records and forwards one event.
    pub(crate) fn emit(&self, event: SwarmEvent) {
        let event = event.scrubbed(&self.redactor);
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let line = json!({
            "seq": state.next,
            "elapsed_ms": self.started.elapsed().as_millis(),
            "event": event,
        });
        state.next += 1;
        if let Err(error) = writeln!(state.file, "{line}") {
            state.failure.get_or_insert(error);
        }
        // The caller may have stopped listening; the file is the record.
        let _ = self.events.send(event);
    }

    /// Flushes the file and reports the first write failure, if any.
    pub(crate) fn finish(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let flushed = state.file.flush();
        match state.failure.take().map_or(flushed, Err) {
            Ok(()) => Ok(()),
            Err(source) => Err(Error::RunDir {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

#[cfg(test)]
mod test;
