//! Unit tests for the swarm's turn hooks.
#![allow(clippy::unwrap_used)]

use std::sync::mpsc;

use openhuman_embed::LastTurnUsage;
use tinyhivemind_hives::Destination;

use super::*;
use crate::event::Redactor;

fn hooks() -> (tempfile::TempDir, SwarmHooks, mpsc::Receiver<SwarmEvent>) {
    let dir = tempfile::tempdir().unwrap();
    let (sender, receiver) = mpsc::channel();
    let trace = Trace::create(
        &dir.path().join("events.jsonl"),
        Redactor::default(),
        sender,
    )
    .unwrap();
    (dir, SwarmHooks::new(Arc::new(trace)), receiver)
}

fn scope(seat: &str) -> TurnScope {
    TurnScope {
        agent_id: seat.into(),
        episode: None,
        message_ids: vec!["brief".into()],
        senders: vec!["host".into()],
        destination: Destination::Agent(seat.into()),
        thread: None,
    }
}

fn started(arguments: serde_json::Value, display_detail: Option<&str>) -> AgentProgress {
    AgentProgress::ToolCallStarted {
        call_id: "c1".into(),
        tool_name: "file_write".into(),
        arguments,
        iteration: 1,
        display_label: None,
        display_detail: display_detail.map(str::to_owned),
    }
}

#[test]
fn a_started_call_prefers_the_display_detail_then_the_arguments() {
    let labelled = tool_event("a", started(serde_json::json!({"path": "x"}), Some("x")));
    assert!(
        matches!(labelled, Some(SwarmEvent::ToolCall { detail: Some(ref d), phase: ToolPhase::Started, .. }) if d == "x")
    );
    let raw = tool_event("a", started(serde_json::json!({"path": "y"}), None));
    assert!(
        matches!(raw, Some(SwarmEvent::ToolCall { detail: Some(ref d), .. }) if d == r#"{"path":"y"}"#)
    );
    let bare = tool_event("a", started(serde_json::Value::Null, None));
    assert!(matches!(
        bare,
        Some(SwarmEvent::ToolCall { detail: None, .. })
    ));
    let long = tool_event("a", started(serde_json::json!("z".repeat(500)), None));
    assert!(
        matches!(long, Some(SwarmEvent::ToolCall { detail: Some(ref d), .. }) if d.chars().count() == DETAIL_CHARS + 1)
    );
}

#[test]
fn a_completed_call_reports_success_and_size() {
    let completed = AgentProgress::ToolCallCompleted {
        call_id: "c1".into(),
        tool_name: "file_write".into(),
        success: false,
        output_chars: 42,
        output: String::new(),
        arguments: None,
        elapsed_ms: 3,
        iteration: 1,
        failure: None,
        display_label: None,
        display_detail: None,
        structured: None,
    };
    assert_eq!(
        tool_event("impl", completed),
        Some(SwarmEvent::ToolCall {
            seat: "impl".into(),
            tool: "file_write".into(),
            phase: ToolPhase::Completed,
            detail: None,
            success: Some(false),
            output_chars: Some(42),
        })
    );
}

#[test]
fn other_progress_is_not_a_tool_event() {
    let thinking = AgentProgress::IterationStarted {
        iteration: 1,
        max_iterations: 4,
    };
    assert_eq!(tool_event("a", thinking), None);
}

#[test]
fn turns_are_tallied_per_seat_and_reported() {
    let (_dir, hooks, receiver) = hooks();
    let usage = LastTurnUsage {
        input_tokens: 100,
        output_tokens: 20,
        cost_usd: Some(0.25),
        ..LastTurnUsage::default()
    };
    hooks.after_turn(&scope("lead"), Some(&usage)).unwrap();
    hooks.after_turn(&scope("lead"), Some(&usage)).unwrap();
    let unknown = LastTurnUsage {
        cost_usd: None,
        ..usage.clone()
    };
    hooks.after_turn(&scope("impl"), Some(&unknown)).unwrap();
    hooks.after_turn(&scope("impl"), None).unwrap();

    let tally = hooks.usage();
    assert_eq!(tally["lead"].turns, 2);
    assert_eq!(tally["lead"].input_tokens, 200);
    assert_eq!(tally["lead"].output_tokens, 40);
    assert!((tally["lead"].cost_usd - 0.5).abs() < f64::EPSILON);
    assert!(tally["lead"].cost_complete);
    assert_eq!(tally["impl"].turns, 2);
    assert!(!tally["impl"].cost_complete);
    let events: Vec<SwarmEvent> = receiver.try_iter().collect();
    assert_eq!(events.len(), 4);
    assert!(
        matches!(&events[3], SwarmEvent::AgentTurn { seat, input_tokens: 0, cost_usd: None, .. } if seat == "impl")
    );
}

#[tokio::test]
async fn progress_streams_tool_calls_into_the_trace() {
    let (_dir, hooks, receiver) = hooks();
    let sink = hooks.progress(&scope("lead")).unwrap();
    sink.send(started(serde_json::Value::Null, Some("NOTES.md")))
        .await
        .unwrap();
    sink.send(AgentProgress::IterationStarted {
        iteration: 1,
        max_iterations: 4,
    })
    .await
    .unwrap();
    drop(sink);
    let event = tokio::task::spawn_blocking(move || receiver.recv().unwrap())
        .await
        .unwrap();
    assert!(
        matches!(event, SwarmEvent::ToolCall { seat, detail: Some(d), .. } if seat == "lead" && d == "NOTES.md")
    );
}
