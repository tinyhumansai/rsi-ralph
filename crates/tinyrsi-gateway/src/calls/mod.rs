//! Upstream call outcomes, tallied in memory as calls finish.
//!
//! The meter writes every call to `meter.jsonl` for later analysis. The
//! tally answers the question a loop asks while it runs: how many calls
//! reached the provider since a given moment, and did the provider refuse
//! them? A provider refusal is an answer with a non-success status (a key
//! over its limit answers `403`, a rate limit `429`) or no complete answer:
//! a connection that fails, or a body that breaks off, even after success
//! headers. A stream is counted when it ends, not when its headers arrive.
//! Calls the gateway refuses itself, such as one over budget, never reach
//! the provider and are not counted.

mod types;

use std::sync::{Mutex, PoisonError};

pub use types::{Refusal, UpstreamCalls};

/// The longest refusal message kept, in characters.
const MESSAGE_CHARS: usize = 300;

/// The running tally behind [`UpstreamCalls`].
#[derive(Debug, Default)]
pub(crate) struct CallTally(Mutex<UpstreamCalls>);

impl CallTally {
    /// Counts a call the provider answered with `status`. A non-success
    /// status is a refusal, whose message is read from `body`. The body
    /// must already be free of the provider key.
    pub(crate) fn answered(&self, status: u16, body: &[u8]) {
        let refusal = (!(200..300).contains(&status)).then(|| Refusal {
            status,
            message: refusal_message(body),
        });
        self.count(refusal);
    }

    /// Counts a call that never got an answer, as a `502` refusal carrying
    /// `error`, which must already be free of the provider key.
    pub(crate) fn unreachable(&self, error: &str) {
        self.count(Some(Refusal {
            status: 502,
            message: shorten(error),
        }));
    }

    /// The tally so far.
    pub(crate) fn snapshot(&self) -> UpstreamCalls {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn count(&self, refusal: Option<Refusal>) {
        let mut calls = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        calls.sent += 1;
        if let Some(refusal) = refusal {
            calls.refused += 1;
            calls.last_refusal = Some(refusal);
        }
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
