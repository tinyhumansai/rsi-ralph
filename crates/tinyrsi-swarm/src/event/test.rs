//! Unit tests for swarm events, their stored form, and secret scrubbing.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use serde_json::json;

use super::*;

#[test]
fn scrubs_every_secret_longest_first() {
    let redactor = Redactor::new(["abc".to_owned(), "abcdef".to_owned(), String::new()]);
    assert_eq!(redactor.scrub("key=abcdef, short=abc"), "key=[redacted], short=[redacted]");
    assert_eq!(Redactor::default().scrub("abc"), "abc");
}

#[test]
fn clips_long_text_on_character_boundaries() {
    assert_eq!(clip("short", 10), "short");
    assert_eq!(clip("ééééé", 2), "éé…");
}

#[test]
fn scrubbing_an_event_touches_only_its_free_text() {
    let redactor = Redactor::new(["s3cret".to_owned()]);
    let message = SwarmEvent::AgentMessage {
        sequence: 1,
        from: "lead".into(),
        to: "implementer-a".into(),
        body: "use s3cret".into(),
        only_for: vec![],
        episode: None,
    }
    .scrubbed(&redactor);
    assert!(matches!(message, SwarmEvent::AgentMessage { ref body, .. } if body == "use [redacted]"));
    let call = SwarmEvent::ToolCall {
        seat: "lead".into(),
        tool: "shell".into(),
        phase: ToolPhase::Started,
        detail: Some("echo s3cret".into()),
        success: None,
        output_chars: None,
    }
    .scrubbed(&redactor);
    assert!(matches!(call, SwarmEvent::ToolCall { detail: Some(ref d), .. } if d == "echo [redacted]"));
    let turn = SwarmEvent::AgentTurn {
        seat: "lead".into(),
        input_tokens: 1,
        output_tokens: 2,
        cost_usd: None,
    };
    assert_eq!(turn.clone().scrubbed(&redactor), turn);
}

#[test]
fn pins_the_stored_form_of_every_event_kind() {
    let started = SwarmEvent::SwarmStarted {
        hive: "pair".into(),
        lead: "lead".into(),
        desks: vec!["lab".into()],
        seats: vec![SeatInfo {
            id: "lead".into(),
            role: "lead".into(),
            model: "m".into(),
            action_dir: PathBuf::from("/incumbent"),
            editing: false,
        }],
    };
    assert_eq!(
        serde_json::to_value(&started).unwrap(),
        json!({"kind": "swarm_started", "hive": "pair", "lead": "lead", "desks": ["lab"],
            "seats": [{"id": "lead", "role": "lead", "model": "m",
                "action_dir": "/incumbent", "editing": false}]})
    );
    let message = SwarmEvent::AgentMessage {
        sequence: 3,
        from: "host".into(),
        to: "hive:lab".into(),
        body: "hi".into(),
        only_for: vec!["lead".into()],
        episode: Some("e1".into()),
    };
    assert_eq!(
        serde_json::to_value(&message).unwrap(),
        json!({"kind": "agent_message", "sequence": 3, "from": "host", "to": "hive:lab",
            "body": "hi", "only_for": ["lead"], "episode": "e1"})
    );
    let call = SwarmEvent::ToolCall {
        seat: "lead".into(),
        tool: "file_read".into(),
        phase: ToolPhase::Completed,
        detail: None,
        success: Some(true),
        output_chars: Some(12),
    };
    assert_eq!(
        serde_json::to_value(&call).unwrap(),
        json!({"kind": "tool_call", "seat": "lead", "tool": "file_read", "phase": "completed",
            "detail": null, "success": true, "output_chars": 12})
    );
    let turn = SwarmEvent::AgentTurn {
        seat: "lead".into(),
        input_tokens: 10,
        output_tokens: 5,
        cost_usd: Some(0.5),
    };
    assert_eq!(
        serde_json::to_value(&turn).unwrap(),
        json!({"kind": "agent_turn", "seat": "lead", "input_tokens": 10,
            "output_tokens": 5, "cost_usd": 0.5})
    );
    let finished = SwarmEvent::SwarmFinished {
        stop: StopReason::WallClock,
        completed: 1,
        failed: 0,
        parked: 0,
    };
    let value = serde_json::to_value(&finished).unwrap();
    assert_eq!(
        value,
        json!({"kind": "swarm_finished", "stop": "wall_clock", "completed": 1,
            "failed": 0, "parked": 0})
    );
    for event in [started, message, call, turn, finished] {
        let round = serde_json::to_value(&event).unwrap();
        assert_eq!(serde_json::from_value::<SwarmEvent>(round).unwrap(), event);
    }
}
