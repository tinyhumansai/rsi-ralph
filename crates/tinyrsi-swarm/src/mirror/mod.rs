//! Mirrors the coordinator transcript into the trace as `agent_message` events.
//!
//! It reads the host-scope transcript, which holds desk posts *and*
//! seat-to-seat direct messages (private rows included); an agent-scope read
//! would miss the direct messages an epoch mostly consists of. A watcher task
//! flushes it periodically and the run flushes it once more at the end. Both
//! share one cursor under one lock, held across read and emit, so a message
//! is never reported twice nor skipped when the final flush races the watcher.

use std::sync::{Mutex, PoisonError};

use tinyhivemind_hives::{Coordinator, Destination, Message};

use crate::event::{SwarmEvent, clip};
use crate::trace::Trace;

/// How much of a message body the trace keeps.
const BODY_CHARS: usize = 4000;

/// The transcript cursor shared by every flush.
#[derive(Debug, Default)]
pub(crate) struct Mirror {
    /// The last sequence reported; `None` before the first message.
    last: Mutex<Option<u64>>,
}

impl Mirror {
    /// Reports every transcript message newer than the cursor.
    ///
    /// # Errors
    ///
    /// The coordinator's failure to read its transcript.
    pub(crate) fn flush(
        &self,
        coordinator: &Coordinator,
        trace: &Trace,
    ) -> tinyhivemind_hives::Result<()> {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        for message in coordinator.read_transcript(*last)? {
            if last.is_some_and(|last| message.sequence <= last) {
                continue;
            }
            *last = Some(message.sequence);
            trace.emit(message_event(message));
        }
        Ok(())
    }
}

/// The trace event for one transcript message.
pub(crate) fn message_event(message: Message) -> SwarmEvent {
    SwarmEvent::AgentMessage {
        sequence: message.sequence,
        from: message.sender,
        to: match message.destination {
            Destination::Hive(id) => format!("hive:{id}"),
            Destination::Agent(id) => id,
        },
        body: clip(&message.body, BODY_CHARS),
        only_for: message.only_for,
        episode: message.episode_id,
    }
}

#[cfg(test)]
mod test;
