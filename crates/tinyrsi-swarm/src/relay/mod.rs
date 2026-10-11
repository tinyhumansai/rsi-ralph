//! Picks up a seat's work when the coordinator goes idle with it hanging.
//!
//! The coordinator goes idle when no delivery is pending, which can happen
//! with the epoch's work unfinished in two ways:
//!
//! - **An unread reply.** A turn that ends with plain text (rather than a
//!   `hivemind_send_agent` call) leaves that text in the transcript as a reply
//!   to the lead, but schedules no turn for the lead. An implementer that runs
//!   out of iterations mid-task reports this way, and nobody reads it.
//! - **An unanswered turn.** A seat's turn on a message ended without the seat
//!   sending anything, typically because the turn failed on a provider error
//!   (a response stream cut off mid-way).
//!
//! In both cases the transcript's last message is addressed to a seat. When
//! the coordinator goes idle on that shape, the swarm sends the seat that
//! should act a direct message from the host quoting the message, which
//! schedules its turn.

use tinyhivemind_hives::{Destination, HOST_ID, Message, SendMessage};

/// How many hanging messages one epoch picks up before it may go idle.
pub(crate) const MAX_RELAYS: usize = 3;

/// A message the epoch is stuck on.
#[derive(Debug, PartialEq)]
pub(crate) enum Stall<'a> {
    /// A seat's reply to the lead that scheduled no lead turn.
    UnreadReply(&'a Message),
    /// A message whose recipient's turn ended without sending anything.
    Unanswered(&'a Message),
}

impl<'a> Stall<'a> {
    /// The seat that should act next.
    pub(crate) fn seat(&self) -> &'a str {
        match self {
            // `stall` only builds a `Stall` for a direct message.
            Stall::UnreadReply(m) | Stall::Unanswered(m) => match &m.destination {
                Destination::Agent(id) | Destination::Hive(id) => id,
            },
        }
    }

    /// The message it should act on.
    pub(crate) fn message(&self) -> &'a Message {
        match self {
            Stall::UnreadReply(m) | Stall::Unanswered(m) => m,
        }
    }
}

/// What the idle transcript is stuck on, if anything: its last message, when
/// that is a direct message to a seat. A seat's message to `lead` is an
/// unread reply; any other (the brief, an assignment) went unanswered. A last
/// word sent to the host or posted to a desk leaves nothing hanging.
pub(crate) fn stall<'a>(transcript: &'a [Message], lead: &str) -> Option<Stall<'a>> {
    let last = transcript.last()?;
    let Destination::Agent(to) = &last.destination else {
        return None;
    };
    if to == HOST_ID {
        return None;
    }
    let from_seat = last.sender != HOST_ID && !last.sender.is_empty() && last.sender != *to;
    Some(if from_seat && to == lead {
        Stall::UnreadReply(last)
    } else {
        Stall::Unanswered(last)
    })
}

/// The host's direct message that hands `stall` back to its seat.
pub(crate) fn relay_message(stall: &Stall<'_>) -> SendMessage {
    let message = stall.message();
    let lead_in = match stall {
        Stall::UnreadReply(m) => format!(
            "{} ended its turn with this reply to you, relayed by the host. If more \
             work is needed, send it with hivemind_send_agent.",
            m.sender
        ),
        Stall::Unanswered(_) => "Your last turn ended before you replied to this message \
             (it may have failed on a provider error). Please continue the task from where \
             you left off, and reply with hivemind_send_agent."
            .to_owned(),
    };
    SendMessage {
        message_id: format!("relay-{}", message.sequence),
        sender: String::new(),
        destination: Destination::Agent(stall.seat().to_owned()),
        body: format!("{lead_in}\n\n{}", message.body),
        thread: None,
        only_for: Vec::new(),
        starters: Vec::new(),
    }
}

#[cfg(test)]
mod test;
