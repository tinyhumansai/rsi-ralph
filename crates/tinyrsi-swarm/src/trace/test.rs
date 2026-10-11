//! Unit tests for the event sink.
#![allow(clippy::unwrap_used)]

use std::sync::mpsc;

use super::*;

fn message(body: &str) -> SwarmEvent {
    SwarmEvent::AgentMessage {
        sequence: 7,
        from: "lead".into(),
        to: "implementer-a".into(),
        body: body.into(),
        only_for: vec![],
        episode: None,
    }
}

#[test]
fn appends_numbered_scrubbed_lines_and_forwards_them() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let (sender, receiver) = mpsc::channel();
    let trace = Trace::create(&path, Redactor::new(["tok-123".to_owned()]), sender).unwrap();
    trace.emit(message("the key is tok-123"));
    trace.emit(message("second"));
    trace.finish().unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("tok-123"));
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["seq"], 0);
    assert_eq!(lines[1]["seq"], 1);
    assert!(lines[0]["elapsed_ms"].is_u64());
    assert_eq!(lines[0]["event"]["body"], "the key is [redacted]");
    let forwarded: Vec<SwarmEvent> = receiver.try_iter().collect();
    assert_eq!(
        forwarded,
        [message("the key is [redacted]"), message("second")]
    );
}

#[test]
fn keeps_recording_after_the_caller_stops_listening() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let (sender, receiver) = mpsc::channel();
    drop(receiver);
    let trace = Trace::create(&path, Redactor::default(), sender).unwrap();
    trace.emit(message("still written"));
    trace.finish().unwrap();
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("still written")
    );
}

#[test]
fn appends_to_an_existing_log() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    std::fs::write(&path, "{\"earlier\":true}\n").unwrap();
    let (sender, _receiver) = mpsc::channel();
    let trace = Trace::create(&path, Redactor::default(), sender).unwrap();
    trace.emit(message("later"));
    trace.finish().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
}

#[test]
fn an_unwritable_log_path_is_a_run_dir_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing/events.jsonl");
    let (sender, _receiver) = mpsc::channel();
    assert!(matches!(
        Trace::create(&path, Redactor::default(), sender),
        Err(Error::RunDir { path: failed, .. }) if failed == path
    ));
}

#[test]
fn a_failed_write_is_reported_when_the_log_is_finished() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let (sender, _receiver) = mpsc::channel();
    let trace = Trace::create(&path, Redactor::default(), sender).unwrap();
    // Swap in a read-only handle so the next append fails.
    trace.state.lock().unwrap().file = std::fs::File::open(&path).unwrap();
    trace.record(message("lost"));
    assert!(matches!(trace.finish(), Err(Error::RunDir { .. })));
}
