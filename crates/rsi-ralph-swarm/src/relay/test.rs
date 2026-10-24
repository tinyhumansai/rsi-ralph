//! Which idle transcripts leave a seat's work hanging, and the host's message
//! that picks it up again.

use tinyhivemind_hives::{Destination, HOST_ID, Message};

use super::{Stall, relay_message, stall};

/// A message sent with an id of its own: the brief, `hivemind_send_agent`.
fn message(sequence: u64, sender: &str, to: &str) -> Message {
    Message {
        scheduled_job_id: None,
        message_id: format!("m{sequence}"),
        sequence,
        sender: sender.into(),
        destination: Destination::Agent(to.into()),
        body: format!("body {sequence}"),
        thread: None,
        episode_id: None,
        only_for: Vec::new(),
    }
}

/// The text a turn ended with, which the coordinator records as a reply.
fn reply(sequence: u64, sender: &str, to: &str) -> Message {
    Message {
        message_id: format!("hivemind:event:{sequence}"),
        ..message(sequence, sender, to)
    }
}

#[test]
fn a_seats_last_word_to_the_lead_is_an_unread_reply() {
    let transcript = [
        message(0, HOST_ID, "lead"),
        message(1, "lead", "implementer-a"),
        reply(2, "implementer-a", "lead"),
    ];
    assert_eq!(
        stall(&transcript, "lead").map(|s| (s.seat().to_owned(), s.message().sequence)),
        Some(("lead".to_owned(), 2))
    );
    assert!(matches!(
        stall(&transcript, "lead"),
        Some(Stall::UnreadReply(_))
    ));
}

#[test]
fn a_message_its_recipient_never_answered_is_an_unanswered_turn() {
    // The lead's turn on the brief failed, or an implementer stopped silently.
    let brief_only = [message(0, HOST_ID, "lead")];
    assert!(matches!(stall(&brief_only, "lead"), Some(Stall::Unanswered(m)) if m.sequence == 0));
    let assignment = [
        message(0, HOST_ID, "lead"),
        message(1, "lead", "implementer-a"),
    ];
    assert_eq!(
        stall(&assignment, "lead").map(|s| (s.seat(), s.message().sequence)),
        Some(("implementer-a", 1))
    );
}

#[test]
fn nothing_hangs_when_the_last_word_went_to_the_host_or_a_desk() {
    let reported = [message(0, HOST_ID, "lead"), reply(1, "lead", HOST_ID)];
    assert!(stall(&reported, "lead").is_none());
    let sent_to_host = [message(0, HOST_ID, "lead"), message(1, "lead", HOST_ID)];
    assert!(stall(&sent_to_host, "lead").is_none());
    let mut desk_post = message(1, "implementer-a", "lead");
    desk_post.destination = Destination::Hive("memory-lab".into());
    assert!(stall(&[desk_post], "lead").is_none());
    assert!(stall(&[], "lead").is_none());
}

#[test]
fn the_leads_closing_reply_to_a_seat_ends_the_epoch() {
    // The lead's final report, written as its turn's reply to the
    // implementer it last heard from, schedules no one and asks for nothing.
    let closed = [
        message(1, "lead", "implementer-a"),
        message(2, "implementer-a", "lead"),
        reply(3, "lead", "implementer-a"),
    ];
    assert!(stall(&closed, "lead").is_none());
    let between_seats = [reply(4, "implementer-a", "reviewer")];
    assert!(stall(&between_seats, "lead").is_none());
}

#[test]
fn the_relay_addresses_the_waiting_seat_and_quotes_the_message() {
    let unread = reply(7, "implementer-a", "lead");
    let relay = relay_message(&Stall::UnreadReply(&unread));
    assert_eq!(relay.message_id, "relay-7");
    assert_eq!(relay.destination, Destination::Agent("lead".into()));
    assert!(relay.body.contains("implementer-a") && relay.body.ends_with("body 7"));
    assert!(relay.body.contains("hivemind_send_agent"));

    let assignment = message(4, "lead", "implementer-a");
    let nudge = relay_message(&Stall::Unanswered(&assignment));
    assert_eq!(nudge.message_id, "relay-4");
    assert_eq!(
        nudge.destination,
        Destination::Agent("implementer-a".into())
    );
    assert!(nudge.body.contains("continue") && nudge.body.ends_with("body 4"));
}
