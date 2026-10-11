//! Relays a seat's last reply to the lead when nothing else will.
//!
//! A turn that ends with plain text (rather than a `hivemind_send_agent`
//! call) leaves that text in the transcript as a reply to whoever sent the
//! turn's message, but the coordinator schedules no turn for the recipient.
//! When an implementer's turn ends that way, typically because it ran out of
//! iterations mid-task, the lead never reads the report and the epoch goes
//! idle with the work unfinished. The swarm checks for that shape whenever the
//! coordinator goes idle and relays the reply to the lead as a direct message
//! from the host, which does schedule its turn.

use tinyhivemind_hives::{Destination, Message, SendMessage};

/// How many replies one epoch relays before it is allowed to go idle.
pub(crate) const MAX_RELAYS: usize = 3;

/// The transcript's last message, if it is a seat's direct message to `lead`:
/// a reply the lead has not acted on. Messages from the host, from the lead,
/// to another seat, or posted to a desk do not count.
pub(crate) fn stranded_reply<'a>(transcript: &'a [Message], lead: &str) -> Option<&'a Message> {
    let last = transcript.last()?;
    let from_seat = last.sender != lead && !last.sender.is_empty() && !last.sender.starts_with("hivemind:");
    let to_lead = matches!(&last.destination, Destination::Agent(id) if id == lead);
    (from_seat && to_lead).then_some(last)
}

/// The host's direct message to `lead` carrying `reply`.
pub(crate) fn relay_message(reply: &Message, lead: &str) -> SendMessage {
    SendMessage {
        message_id: format!("relay-{}", reply.sequence),
        sender: String::new(),
        destination: Destination::Agent(lead.into()),
        body: format!(
            "{} ended its turn with this reply to you, relayed by the host. If more \
             work is needed, send it with hivemind_send_agent.\n\n{}",
            reply.sender, reply.body
        ),
        thread: None,
        only_for: Vec::new(),
        starters: Vec::new(),
    }
}

#[cfg(test)]
mod test;
