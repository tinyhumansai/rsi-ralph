//! Upstream call outcomes, tallied in memory as calls are sent and answered.
//!
//! The meter writes every call to `meter.jsonl` for later analysis. The
//! tally answers the question a loop asks while it runs: how many calls
//! reached the provider since a given moment, and did the provider refuse
//! them? A provider refusal is an answer with a non-success status (a key
//! over its limit answers `403`, a rate limit `429`) or no complete answer:
//! a connection that fails, or a body that breaks off, even after success
//! headers. A call counts as sent when it is forwarded; a stream's refusal is
//! known only once its body ends, and is kept with the call it answers.
//! Calls the gateway refuses itself, such as one over budget, never reach
//! the provider and are not counted.

mod types;

use std::sync::{Mutex, PoisonError};

pub use types::{CallMark, Refusal, UpstreamCalls};

/// The longest refusal message kept, in characters.
const MESSAGE_CHARS: usize = 300;

/// The running tally behind [`UpstreamCalls`]: how many calls were
/// dispatched, and each refusal with the position of the call it answers.
///
/// A refusal is kept with its call's position rather than counted when it
/// arrives, so a call sent in one phase and refused during the next is
/// never charged to the later phase.
#[derive(Debug, Default)]
pub(crate) struct CallTally(Mutex<Tally>);

#[derive(Debug, Default)]
struct Tally {
    sent: u64,
    refusals: Vec<(u64, Refusal)>,
}

impl CallTally {
    /// Counts a call as sent, as it is forwarded, and returns its position
    /// for recording its answer. Counting before the answer means a reading
    /// taken while a call is still waiting on the provider includes it.
    pub(crate) fn dispatched(&self) -> u64 {
        let mut tally = self.lock();
        tally.sent += 1;
        tally.sent - 1
    }

    /// The mark after every call dispatched so far.
    pub(crate) fn mark(&self) -> CallMark {
        CallMark(self.lock().sent)
    }

    /// Records the answer to call `call`: `status`, with `body`. A
    /// non-success status is a refusal, whose message is read from `body`.
    /// The body must already be free of the provider key.
    pub(crate) fn answered(&self, call: u64, status: u16, body: &[u8]) {
        if !(200..300).contains(&status) {
            self.refuse(
                call,
                Refusal {
                    status,
                    message: refusal_message(body),
                },
            );
        }
    }

    /// Records that call `call` got no complete answer, as a `502` refusal
    /// carrying `error`, which must already be free of the provider key.
    pub(crate) fn unreachable(&self, call: u64, error: &str) {
        self.refuse(
            call,
            Refusal {
                status: 502,
                message: shorten(error),
            },
        );
    }

    /// The calls dispatched at or after `mark`, and what is known so far of
    /// their refusals.
    pub(crate) fn since(&self, mark: CallMark) -> UpstreamCalls {
        let tally = self.lock();
        let mut refused = 0;
        let mut last_refusal = None;
        for (call, refusal) in &tally.refusals {
            if *call >= mark.0 {
                refused += 1;
                last_refusal = Some(refusal);
            }
        }
        UpstreamCalls {
            sent: tally.sent.saturating_sub(mark.0),
            refused,
            last_refusal: last_refusal.cloned(),
        }
    }

    fn refuse(&self, call: u64, refusal: Refusal) {
        self.lock().refusals.push((call, refusal));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Tally> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The provider's own words from a refusal body: an OpenAI-style
/// `error.message`, else a string `error`, else the body text, on one line
/// and at most [`MESSAGE_CHARS`] long.
pub(crate) fn refusal_message(body: &[u8]) -> String {
    let parsed = serde_json::from_slice::<serde_json::Value>(body).ok();
    let error = parsed.as_ref().map(|v| &v["error"]);
    let text = error
        .and_then(|e| e["message"].as_str().or_else(|| e.as_str()))
        .map_or_else(|| String::from_utf8_lossy(body).into_owned(), str::to_owned);
    shorten(&text)
}

/// `text` with whitespace runs collapsed to one space, cut to
/// [`MESSAGE_CHARS`] with a trailing ellipsis when longer.
fn shorten(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= MESSAGE_CHARS {
        return line;
    }
    let mut cut: String = line.chars().take(MESSAGE_CHARS - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod test;
