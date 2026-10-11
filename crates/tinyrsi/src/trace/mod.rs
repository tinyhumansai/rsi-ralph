//! Append-only loop event stream that a separate process can tail without locks.
//!
//! A run emits [`LoopEvent`]s through a [`TraceSink`]. Each sink stamps the
//! event into a [`TraceRecord`] with a gap-free sequence number and the
//! milliseconds reported by an injected [`Clock`], so tests stay deterministic
//! with a [`ManualClock`]. [`JsonlTrace`] writes one flushed JSON line per
//! event to `events.jsonl`; [`VecTrace`] keeps records in memory; [`NullTrace`]
//! discards them. The wire schema is specified in `docs/specs/loop-trace.md`
//! and described in this module's `README.md`.
mod types;
use crate::{Error, Result};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
pub use types::*;

/// Source of elapsed run time.
pub trait Clock: Send {
    /// Milliseconds since the run started.
    fn elapsed_ms(&self) -> u64;
}

/// Wall-clock time since construction, from the monotonic system clock.
#[derive(Debug, Clone, Copy)]
pub struct MonotonicClock {
    start: Instant,
}

impl MonotonicClock {
    /// Starts counting now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for MonotonicClock {
    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// A clock that moves only when told to; clones share the same time.
#[derive(Debug, Clone, Default)]
pub struct ManualClock {
    now: Arc<AtomicU64>,
}

impl ManualClock {
    /// Starts at zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves time forward by `ms` milliseconds.
    pub fn advance(&self, ms: u64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn elapsed_ms(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
}

/// Destination of loop events.
pub trait TraceSink {
    /// Stamps and records one event.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TraceIo`] when the event cannot be recorded.
    fn record(&mut self, event: LoopEvent) -> Result<()>;
}

/// Sequence and time stamping shared by the recording sinks.
struct Stamp {
    seq: u64,
    clock: Box<dyn Clock>,
}

impl Stamp {
    fn next(&mut self, event: LoopEvent) -> TraceRecord {
        let record = TraceRecord {
            seq: self.seq,
            elapsed_ms: self.clock.elapsed_ms(),
            event,
        };
        self.seq += 1;
        record
    }
}

/// Writes `events.jsonl`: one JSON object per line, flushed after each line.
pub struct JsonlTrace {
    file: File,
    stamp: Stamp,
}

impl JsonlTrace {
    /// Creates a fresh trace file, creating missing parent directories.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TraceIo`] when the file already exists or cannot be
    /// created. A trace is never appended to across runs, so `seq` always
    /// starts at 0.
    pub fn create(path: impl AsRef<Path>, clock: impl Clock + 'static) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|_| Error::TraceIo)?;
        }
        let file = OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(path)
            .map_err(|_| Error::TraceIo)?;
        Ok(Self {
            file,
            stamp: Stamp {
                seq: 0,
                clock: Box::new(clock),
            },
        })
    }
}

impl TraceSink for JsonlTrace {
    fn record(&mut self, event: LoopEvent) -> Result<()> {
        let mut line = serde_json::to_vec(&self.stamp.next(event)).map_err(|_| Error::TraceIo)?;
        line.push(b'\n');
        // One write per line keeps a concurrent reader from seeing interleaved
        // fragments; the flush makes the line visible before the next stage.
        self.file.write_all(&line).map_err(|_| Error::TraceIo)?;
        self.file.flush().map_err(|_| Error::TraceIo)
    }
}

impl std::fmt::Debug for JsonlTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonlTrace")
            .field("seq", &self.stamp.seq)
            .finish_non_exhaustive()
    }
}

/// Keeps stamped records in memory, for tests and embedding hosts.
pub struct VecTrace {
    records: Vec<TraceRecord>,
    stamp: Stamp,
}

impl VecTrace {
    /// Starts empty with the given clock.
    #[must_use]
    pub fn new(clock: impl Clock + 'static) -> Self {
        Self {
            records: vec![],
            stamp: Stamp {
                seq: 0,
                clock: Box::new(clock),
            },
        }
    }

    /// Records so far, in order.
    #[must_use]
    pub fn records(&self) -> &[TraceRecord] {
        &self.records
    }
}

impl TraceSink for VecTrace {
    fn record(&mut self, event: LoopEvent) -> Result<()> {
        let record = self.stamp.next(event);
        self.records.push(record);
        Ok(())
    }
}

impl std::fmt::Debug for VecTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VecTrace")
            .field("records", &self.records.len())
            .finish_non_exhaustive()
    }
}

/// Discards every event.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullTrace;

impl TraceSink for NullTrace {
    fn record(&mut self, _: LoopEvent) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod test;
