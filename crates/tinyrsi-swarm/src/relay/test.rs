//! Which idle transcripts end in a reply the lead never got, and the relay.

use tinyhivemind_hives::{Destination, Message};

use super::{relay_message, stranded_reply};

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

#[test]
fn a_seats_last_word_to_the_lead_is_stranded() {
    let transcript = [
        message(0, "hivemind:host", "lead"),
        message(1, "lead", "implementer-a"),
        message(2, "implementer-a", "lead"),
    ];
    assert_eq!(
        stranded_reply(&transcript, "lead").map(|m| m.sequence),
        Some(2)
    );
}

#[test]
fn nothing_is_stranded_when_the_lead_spoke_last_or_the_reply_went_elsewhere() {
    let lead_last = [
        message(1, "implementer-a", "lead"),
        message(2, "lead", "hivemind:host"),
    ];
    assert!(stranded_reply(&lead_last, "lead").is_none());
    let to_someone_else = [message(1, "implementer-a", "reviewer")];
    assert!(stranded_reply(&to_someone_else, "lead").is_none());
    let from_the_host = [message(1, "hivemind:host", "lead")];
    assert!(stranded_reply(&from_the_host, "lead").is_none());
    let mut desk_post = message(1, "implementer-a", "lead");
    desk_post.destination = Destination::Hive("memory-lab".into());
    assert!(stranded_reply(&[desk_post], "lead").is_none());
    assert!(stranded_reply(&[], "lead").is_none());
}

#[test]
fn the_relay_is_a_direct_message_to_the_lead_quoting_the_reply() {
    let reply = message(7, "implementer-a", "lead");
    let relay = relay_message(&reply, "lead");
    assert_eq!(relay.message_id, "relay-7");
    assert_eq!(relay.destination, Destination::Agent("lead".into()));
    assert!(relay.body.contains("implementer-a"));
    assert!(relay.body.ends_with("body 7"));
    assert!(relay.body.contains("hivemind_send_agent"));
}
