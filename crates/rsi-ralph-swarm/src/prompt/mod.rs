//! Each seat's system prompt: seat prompt, role charter, working directory,
//! shared context, and the rules of working in the hive.
//!
//! The closing rule is a lesson from the first live run: a seat that waited
//! or polled for a reply inside its own turn held its coordinator slot until
//! the turn wall, and the peer it waited for was never scheduled. Turns are
//! asynchronous, so every prompt says to end the turn after sending.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Appended to every seat's system prompt.
pub const HIVE_RULES: &str = "## Working in the hive

You are one seat of a hive. Talk to other seats with hivemind_send_agent; \
read your conversation with a seat with hivemind_read. The host's brief \
arrives as a direct message to the lead. The lead ends the epoch by sending \
no further messages and replying with its final report.

Turns are asynchronous. After you send a message, END YOUR TURN with a \
one-line reply and no further tool calls; you are woken automatically when \
someone replies. Never wait, sleep or poll for replies.
";

/// What a seat's prompt is composed from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PromptParts<'a> {
    /// The seat's own prompt body.
    pub seat: &'a str,
    /// The role it fills.
    pub role: &'a str,
    /// That role's charter body.
    pub charter: &'a str,
    /// The checkout the seat works in.
    pub action_dir: &'a Path,
    /// Whether that checkout is the seat's own editable candidate.
    pub editing: bool,
    /// Shared context documents, path → text.
    pub context: &'a BTreeMap<String, String>,
}

/// Composes the system prompt for one seat.
pub(crate) fn system_prompt(parts: &PromptParts<'_>) -> String {
    let access = if parts.editing {
        "your own candidate checkout; edit it freely"
    } else {
        "the incumbent checkout; read-only"
    };
    let mut text = format!(
        "{}\n\n## Your role: {}\n\n{}\n\n## Your working directory\n\n{} ({access})\n",
        parts.seat.trim(),
        parts.role,
        parts.charter.trim(),
        parts.action_dir.display(),
    );
    for (path, body) in parts.context {
        // Writing to a `String` cannot fail.
        let _ = write!(text, "\n## Shared context: {path}\n\n{}\n", body.trim());
    }
    text.push('\n');
    text.push_str(HIVE_RULES);
    text
}

#[cfg(test)]
mod test;
