//! What a running swarm reports: the events streamed to the caller's
//! progress callback and appended to `events.jsonl`.
//!
//! Each `events.jsonl` line is `{"seq": n, "elapsed_ms": t, "event": {...}}`,
//! the envelope `apps/rsi-viz` follows, with the event tagged by `kind`:
//!
//! | kind | fields |
//! | --- | --- |
//! | `swarm_started` | `hive`, `lead`, `desks`, `seats` (id, role, model, `action_dir`, editing) |
//! | `agent_message` | `sequence`, `from`, `to` (`hive:<id>` or a seat), `body`, `only_for`, `episode` |
//! | `tool_call` | `seat`, `tool`, `phase` (`started`/`completed`), `detail`, `success`, `output_chars` |
//! | `agent_turn` | `seat`, `input_tokens`, `output_tokens`, `cost_usd` |
//! | `swarm_finished` | `stop` (`idle`/`wall_clock`), `completed`, `failed`, `parked` |

mod types;

pub use types::{SeatInfo, StopReason, SwarmEvent, ToolPhase};

/// Removes every known secret from text before it leaves the swarm.
///
/// Tool keys and the model key are configuration, but a model can echo an
/// environment variable or a tool's output can quote a header. Every event
/// passes through one [`Redactor`] before it reaches `events.jsonl` or the
/// caller, so a secret never does.
#[derive(Clone, Debug, Default)]
pub(crate) struct Redactor {
    secrets: Vec<String>,
}

/// What a redacted secret is replaced with.
pub(crate) const REDACTED: &str = "[redacted]";

impl Redactor {
    /// A redactor for `secrets`; empty strings are ignored, and longer
    /// secrets are replaced first so one containing another is fully hidden.
    pub(crate) fn new(secrets: impl IntoIterator<Item = String>) -> Self {
        let mut secrets: Vec<String> = secrets.into_iter().filter(|s| !s.is_empty()).collect();
        secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
        secrets.dedup();
        Self { secrets }
    }

    /// `text` with every secret replaced by [`REDACTED`].
    pub(crate) fn scrub(&self, text: &str) -> String {
        self.secrets.iter().fold(text.to_owned(), |text, secret| {
            text.replace(secret, REDACTED)
        })
    }
}

/// Shortens `text` to at most `max` characters, marking the cut with `…`.
pub(crate) fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod test;
