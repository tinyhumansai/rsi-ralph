//! End-to-end swarm runs against a loopback, scripted OpenAI-compatible model.
//!
//! Each seat's model calls go to its own path on one wiremock server, so the
//! script per seat is deterministic: the lead assigns work over a direct
//! message, the implementer writes a file into its candidate checkout. No
//! network, no wall-clock dependence beyond generous bounds.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tinyrsi_swarm::{
    EVENTS_FILE, HIVE_DB, StopReason, SwarmEvent, SwarmRequest, ToolPhase, editing_seats,
    load_package, run_swarm,
};
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const MODEL_KEY: &str = "sk-fixture-model-key-1234";
const NOTE: &str = "the swarm was here\n";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair")
}

/// A chat completion carrying `message`.
fn completion(message: Value) -> Value {
    let finish = if message.get("tool_calls").is_some() {
        "tool_calls"
    } else {
        "stop"
    };
    json!({
        "id": "fixture", "object": "chat.completion", "created": 0, "model": "fixture",
        "choices": [{"index": 0, "message": message, "finish_reason": finish}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
    })
}

fn tool_call(id: &str, name: &str, arguments: &Value) -> Value {
    completion(json!({
        "role": "assistant", "content": null,
        "tool_calls": [{"id": id, "type": "function",
            "function": {"name": name, "arguments": arguments.to_string()}}]
    }))
}

fn text(content: &str) -> Value {
    completion(json!({"role": "assistant", "content": content}))
}

/// Replies in order, then repeats the last reply forever.
struct Script {
    replies: Vec<Value>,
    calls: AtomicUsize,
    delay: Duration,
}

impl Respond for Script {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let reply = self
            .replies
            .get(call)
            .or_else(|| self.replies.last())
            .cloned()
            .unwrap();
        ResponseTemplate::new(200)
            .set_delay(self.delay)
            .set_body_json(reply)
    }
}

async fn mount(server: &MockServer, seat: &str, replies: Vec<Value>, delay: Duration) {
    Mock::given(method("POST"))
        .and(path_regex(format!("^/{seat}/v1/chat/completions$")))
        .respond_with(Script {
            replies,
            calls: AtomicUsize::new(0),
            delay,
        })
        .mount(server)
        .await;
}

/// The scripted pair: the lead assigns, the implementer writes `NOTES.md`.
async fn model(implementer_delay: Duration) -> MockServer {
    let server = MockServer::start().await;
    let assign = json!({"agent_id": "implementer-a", "message_id": "assign-1",
        "body": "Write NOTES.md in your candidate checkout."});
    mount(
        &server,
        "lead",
        vec![
            tool_call("call-lead-1", "hivemind_send_agent", &assign),
            text("Assigned implementer-a; ending my turn."),
        ],
        Duration::ZERO,
    )
    .await;
    let write = json!({"path": "NOTES.md", "content": NOTE});
    mount(
        &server,
        "implementer-a",
        vec![
            tool_call("call-impl-1", "file_write", &write),
            text("Wrote NOTES.md."),
        ],
        implementer_delay,
    )
    .await;
    mount(&server, "runtime", vec![text("ok")], Duration::ZERO).await;
    server
}

struct Dirs {
    _root: tempfile::TempDir,
    run: PathBuf,
    incumbent: PathBuf,
    candidate: PathBuf,
}

fn dirs() -> Dirs {
    let root = tempfile::tempdir().unwrap();
    let incumbent = root.path().join("incumbent");
    let candidate = root.path().join("candidates/implementer-a");
    std::fs::create_dir_all(&incumbent).unwrap();
    std::fs::create_dir_all(&candidate).unwrap();
    std::fs::write(incumbent.join("README.md"), "incumbent\n").unwrap();
    Dirs {
        run: root.path().join("run"),
        incumbent,
        candidate,
        _root: root,
    }
}

fn request(dirs: &Dirs, server_uri: &str, wall: Duration) -> SwarmRequest {
    let package = load_package(&fixture()).unwrap();
    assert_eq!(editing_seats(&package).unwrap(), ["implementer-a"]);
    let base = server_uri.to_owned();
    SwarmRequest {
        package,
        lead: "lead".into(),
        brief: "Leave a note in the candidate.".into(),
        run_dir: dirs.run.clone(),
        incumbent: dirs.incumbent.clone(),
        candidates: BTreeMap::from([("implementer-a".into(), dirs.candidate.clone())]),
        model_base_url: Arc::new(move |seat| format!("{base}/{seat}/v1")),
        model_api_key: MODEL_KEY.into(),
        default_model: "fixture-default".into(),
        temperature: Some(0.0),
        tools: Vec::new(),
        wall,
        turn: Duration::from_secs(60),
    }
}

/// Every file under `dir`, recursively.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(files(&path));
        } else {
            found.push(path);
        }
    }
    found
}

#[test]
fn the_lead_delegates_and_the_implementer_writes_its_candidate() {
    let server_runtime = tokio::runtime::Runtime::new().unwrap();
    let server = server_runtime.block_on(model(Duration::ZERO));
    let dirs = dirs();
    let mut events = Vec::new();
    let outcome = run_swarm(
        request(&dirs, &server.uri(), Duration::from_secs(120)),
        &mut |event| events.push(event),
    )
    .unwrap();

    assert_eq!(outcome.stop, StopReason::Idle);
    assert_eq!(outcome.failed, 0);
    assert_eq!(
        std::fs::read_to_string(dirs.candidate.join("NOTES.md")).unwrap(),
        NOTE
    );
    assert!(!dirs.incumbent.join("NOTES.md").exists());
    assert!(outcome.seats["lead"].turns >= 1);
    assert!(outcome.seats["implementer-a"].turns >= 1);
    assert!(outcome.seats["implementer-a"].input_tokens > 0);

    // The brief, then the lead's direct message to the implementer.
    let messages: Vec<(&str, &str)> = events
        .iter()
        .filter_map(|event| match event {
            SwarmEvent::AgentMessage { from, to, .. } => Some((from.as_str(), to.as_str())),
            _ => None,
        })
        .collect();
    assert!(messages.iter().any(|(_, to)| *to == "lead"), "{messages:?}");
    assert!(messages.contains(&("lead", "implementer-a")), "{messages:?}");
    assert!(events.iter().any(|event| matches!(event,
        SwarmEvent::ToolCall { seat, tool, phase: ToolPhase::Completed, success: Some(true), .. }
            if seat == "implementer-a" && tool == "file_write")));
    assert!(matches!(events.first(), Some(SwarmEvent::SwarmStarted { lead, .. }) if lead == "lead"));
    assert!(matches!(
        events.last(),
        Some(SwarmEvent::SwarmFinished { stop: StopReason::Idle, .. })
    ));

    // `events.jsonl` holds the same events, in the documented envelope.
    let log = std::fs::read_to_string(dirs.run.join(EVENTS_FILE)).unwrap();
    let lines: Vec<Value> = log.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(lines.len(), events.len());
    assert_eq!(lines[0]["seq"], 0);
    assert_eq!(lines[0]["event"]["kind"], "swarm_started");
    assert!(dirs.run.join(HIVE_DB).exists());

    assert_no_file_contains(&dirs.run, MODEL_KEY);

    // Each seat's prompt carries the hive rules; nobody is offered `wait`.
    let requests = server_runtime.block_on(server.received_requests()).unwrap();
    let lead: Value = requests
        .iter()
        .find(|request| request.url.path() == "/lead/v1/chat/completions")
        .map(|request| serde_json::from_slice(&request.body).unwrap())
        .unwrap();
    assert!(lead.to_string().contains("END YOUR TURN"));
    let offered: Vec<&str> = lead["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect();
    assert!(offered.contains(&"hivemind_send_agent"), "{offered:?}");
    assert!(!offered.contains(&"wait") && !offered.contains(&"wait_loop"), "{offered:?}");
}

#[test]
fn the_wall_clock_stops_a_swarm_whose_seat_is_still_working() {
    let server_runtime = tokio::runtime::Runtime::new().unwrap();
    let server = server_runtime.block_on(model(Duration::from_secs(30)));
    let dirs = dirs();
    let mut events = Vec::new();
    let outcome = run_swarm(
        request(&dirs, &server.uri(), Duration::from_secs(3)),
        &mut |event| events.push(event),
    )
    .unwrap();

    assert_eq!(outcome.stop, StopReason::WallClock);
    assert!(!dirs.candidate.join("NOTES.md").exists());
    assert!(matches!(
        events.last(),
        Some(SwarmEvent::SwarmFinished { stop: StopReason::WallClock, .. })
    ));
}

#[test]
fn a_lead_outside_the_package_is_refused_before_anything_starts() {
    let dirs = dirs();
    let mut request = request(&dirs, "http://127.0.0.1:9", Duration::from_secs(1));
    request.lead = "nobody".into();
    let error = run_swarm(request, &mut |_| {}).unwrap_err();
    assert!(matches!(error, tinyrsi_swarm::Error::UnknownLead(ref lead) if lead == "nobody"));
    assert!(!dirs.run.exists());
}

#[test]
fn run_failures_surface_as_typed_errors_without_the_model_key() {
    // A zero turn wall is refused by the hive adapter.
    let zero_dirs = dirs();
    let mut zero_turn = request(&zero_dirs, "http://127.0.0.1:9", Duration::from_secs(1));
    zero_turn.turn = Duration::ZERO;
    let error = run_swarm(zero_turn, &mut |_| {}).unwrap_err();
    assert!(matches!(error, tinyrsi_swarm::Error::Adapter(_)), "{error:?}");

    // A coordinator database that cannot be opened.
    let db_dirs = dirs();
    std::fs::create_dir_all(db_dirs.run.join(HIVE_DB)).unwrap();
    let error = run_swarm(
        request(&db_dirs, "http://127.0.0.1:9", Duration::from_secs(1)),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(error, tinyrsi_swarm::Error::Coordinator(_)), "{error:?}");

    // A seat id OpenHuman reserves for its own agents.
    let reserved_dirs = dirs();
    let mut reserved = request(&reserved_dirs, "http://127.0.0.1:9", Duration::from_secs(1));
    let hive = reserved.package.get_mut("hive.json").unwrap();
    *hive = hive.replace("\"implementer-a\"", "\"orchestrator\"");
    let candidate = reserved.candidates.remove("implementer-a").unwrap();
    reserved.candidates.insert("orchestrator".into(), candidate);
    let error = run_swarm(reserved, &mut |_| {}).unwrap_err();
    assert!(
        matches!(error, tinyrsi_swarm::Error::Agent { ref seat, .. } if seat == "orchestrator"),
        "{error:?}"
    );
    assert!(!format!("{error:?}").contains(MODEL_KEY));

    // A run directory that is a file.
    let file_dirs = dirs();
    std::fs::write(&file_dirs.run, "not a directory").unwrap();
    let error = run_swarm(
        request(&file_dirs, "http://127.0.0.1:9", Duration::from_secs(1)),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(error, tinyrsi_swarm::Error::RunDir { .. }), "{error:?}");
}
