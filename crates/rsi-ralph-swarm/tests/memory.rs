//! Seat memory end to end: swarm runs against a scripted model and
//! `TinyMemory`'s in-process reference engine.
//!
//! Its own test binary, because `OpenHuman` installs a memory engine for the
//! whole process: the runs here would otherwise leak memory into the other
//! swarm tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rsi_ralph_swarm::{
    SHARED_MEMORY_AGENT, StopReason, SwarmEvent, SwarmMemory, SwarmRequest, load_package, run_swarm,
};
use serde_json::{Value, json};
use tinymemory_api::conformance::reference::ReferenceEngine;
use tinymemory_api::{ListRequest, MemoryEngine, MetaFilter};
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const ROOT: &str = "team:pair";
/// What the implementer reports in the first run; the second run's seats
/// must be handed it from memory.
const FIRST_RUN_FINDING: &str = "Finding: the sieve bound fails below n = 7.";

static ONE_SWARM_AT_A_TIME: Mutex<()> = Mutex::new(());

fn one_swarm_at_a_time() -> MutexGuard<'static, ()> {
    ONE_SWARM_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn completion(message: &Value) -> Value {
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
    completion(&json!({
        "role": "assistant", "content": null,
        "tool_calls": [{"id": id, "type": "function",
            "function": {"name": name, "arguments": arguments.to_string()}}]
    }))
}

fn text(content: &str) -> Value {
    completion(&json!({"role": "assistant", "content": content}))
}

/// Replies in order, then repeats the last reply forever.
struct Script {
    replies: Vec<Value>,
    calls: AtomicUsize,
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
        ResponseTemplate::new(200).set_body_json(reply)
    }
}

async fn mount(server: &MockServer, seat: &str, replies: Vec<Value>) {
    Mock::given(method("POST"))
        .and(path_regex(format!("^/{seat}/v1/chat/completions$")))
        .respond_with(Script {
            replies,
            calls: AtomicUsize::new(0),
        })
        .mount(server)
        .await;
}

/// The lead assigns; the implementer writes `NOTES.md` and reports `report`.
async fn model(report: &str) -> MockServer {
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
    )
    .await;
    let write = json!({"path": "NOTES.md", "content": "notes\n"});
    mount(
        &server,
        "implementer-a",
        vec![tool_call("call-impl-1", "file_write", &write), text(report)],
    )
    .await;
    mount(&server, "runtime", vec![text("ok")]).await;
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
    Dirs {
        run: root.path().join("run"),
        incumbent,
        candidate,
        _root: root,
    }
}

fn request(dirs: &Dirs, server_uri: &str, memory: SwarmMemory) -> SwarmRequest {
    let package =
        load_package(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair")).unwrap();
    let base = server_uri.to_owned();
    SwarmRequest {
        package,
        lead: "lead".into(),
        brief: "Leave a note in the candidate.".into(),
        run_dir: dirs.run.clone(),
        incumbent: dirs.incumbent.clone(),
        candidates: BTreeMap::from([("implementer-a".into(), dirs.candidate.clone())]),
        model_base_url: Arc::new(move |seat| format!("{base}/{seat}/v1")),
        model_api_key: "sk-fixture-model-key".into(),
        default_model: "fixture-default".into(),
        temperature: Some(0.0),
        tools: Vec::new(),
        memory: Some(memory),
        wall: Duration::from_secs(120),
        turn: Duration::from_secs(60),
    }
}

/// One swarm run; returns its events and every model request it made.
fn run(memory: SwarmMemory, report: &str) -> (Vec<SwarmEvent>, Vec<Request>) {
    let server_runtime = tokio::runtime::Runtime::new().unwrap();
    let server = server_runtime.block_on(model(report));
    let dirs = dirs();
    let mut events = Vec::new();
    let outcome = run_swarm(request(&dirs, &server.uri(), memory), &mut |event| {
        events.push(event);
    })
    .unwrap();
    assert_eq!(outcome.stop, StopReason::Idle);
    assert_eq!(outcome.failed, 0);
    let requests = server_runtime.block_on(server.received_requests()).unwrap();
    (events, requests)
}

/// Every namespace the engine holds an item in.
fn namespaces(engine: &ReferenceEngine) -> BTreeSet<String> {
    let page = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(engine.list(ListRequest::new(MetaFilter::default(), 1000)))
        .unwrap();
    page.items
        .iter()
        .map(|hit| hit.meta.namespace.to_string())
        .collect()
}

/// The memory namespace each seat was announced with.
fn announced(events: &[SwarmEvent]) -> BTreeMap<String, Option<String>> {
    match events.first() {
        Some(SwarmEvent::SwarmStarted { seats, .. }) => seats
            .iter()
            .map(|seat| (seat.id.clone(), seat.memory.clone()))
            .collect(),
        other => panic!("the run did not start with swarm_started: {other:?}"),
    }
}

fn offered(requests: &[Request], seat: &str) -> Vec<String> {
    let path = format!("/{seat}/v1/chat/completions");
    let body: Value = requests
        .iter()
        .find(|request| request.url.path() == path)
        .map(|request| serde_json::from_slice(&request.body).unwrap())
        .unwrap();
    body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn shared_memory_is_one_namespace_for_every_seat_and_every_run() {
    let _turn = one_swarm_at_a_time();
    let engine = Arc::new(ReferenceEngine::new());
    let memory = SwarmMemory::new(engine.clone(), ROOT).unwrap().shared(true);
    let shared = format!("{ROOT}/agent:{SHARED_MEMORY_AGENT}");

    let (events, requests) = run(memory.clone(), FIRST_RUN_FINDING);
    assert_eq!(
        announced(&events),
        BTreeMap::from([
            ("lead".into(), Some(shared.clone())),
            ("implementer-a".into(), Some(shared.clone())),
        ])
    );
    for seat in ["lead", "implementer-a"] {
        assert!(
            offered(&requests, seat).iter().any(|tool| tool == "memory"),
            "{seat} was not offered the memory tool"
        );
    }
    let written = namespaces(&engine);
    assert!(written.contains(&shared), "{written:?}");
    for seat in ["lead", "implementer-a"] {
        assert!(
            !written.contains(&format!("{ROOT}/agent:{seat}")),
            "{seat} kept a namespace of its own: {written:?}"
        );
    }

    // A later run on the same memory starts from what the first one logged.
    let (_, requests) = run(memory, "Nothing new.");
    assert!(
        requests
            .iter()
            .any(|request| String::from_utf8_lossy(&request.body).contains(FIRST_RUN_FINDING)),
        "no seat of the second run was given the first run's finding"
    );
}

#[test]
fn without_sharing_each_seat_logs_to_its_own_namespace() {
    let _turn = one_swarm_at_a_time();
    let engine = Arc::new(ReferenceEngine::new());
    let memory = SwarmMemory::new(engine.clone(), ROOT).unwrap();

    let (events, _) = run(memory, FIRST_RUN_FINDING);
    assert_eq!(
        announced(&events),
        BTreeMap::from([
            ("lead".into(), Some(format!("{ROOT}/agent:lead"))),
            (
                "implementer-a".into(),
                Some(format!("{ROOT}/agent:implementer-a"))
            ),
        ])
    );
    let written = namespaces(&engine);
    for seat in ["lead", "implementer-a"] {
        assert!(
            written.contains(&format!("{ROOT}/agent:{seat}")),
            "{seat} logged nothing of its own: {written:?}"
        );
    }
    assert!(!written.contains(&format!("{ROOT}/agent:{SHARED_MEMORY_AGENT}")));
}

/// The same shared run against a real `CortexDB` server: set
/// `RSI_RALPH_LIVE_CORTEXDB_URL` and `RSI_RALPH_LIVE_CORTEXDB_KEY` to run it.
/// Each run writes below a fresh root, so earlier runs cannot satisfy it.
#[test]
fn live_shared_memory_lands_in_one_cortexdb_namespace() {
    let (Ok(url), Ok(key)) = (
        std::env::var("RSI_RALPH_LIVE_CORTEXDB_URL"),
        std::env::var("RSI_RALPH_LIVE_CORTEXDB_KEY"),
    ) else {
        eprintln!("skipped: RSI_RALPH_LIVE_CORTEXDB_URL and _KEY are not set");
        return;
    };
    let _turn = one_swarm_at_a_time();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let root = format!("team:rsi-live-{stamp}");
    let memory = SwarmMemory::cortexdb(&url, &key, root.clone())
        .unwrap()
        .shared(true);
    let (events, _) = run(memory, FIRST_RUN_FINDING);
    let shared = format!("{root}/agent:{SHARED_MEMORY_AGENT}");
    assert!(announced(&events).values().all(|ns| ns.as_deref() == Some(shared.as_str())));

    // Read back through a fresh client: what the seats wrote, where.
    let reader = tinymemory_integrations::build_engine(
        tinymemory_integrations::CORTEXDB_ENGINE_ID,
        &tinymemory_integrations::EngineSettings {
            endpoint: Some(url),
            ..tinymemory_integrations::EngineSettings::default()
        },
        tinymemory_integrations::EngineCredential::Static(key),
    )
    .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut seen = BTreeSet::new();
    for _ in 0..30 {
        let page = runtime
            .block_on(reader.list(ListRequest::new(MetaFilter::default(), 1000)))
            .unwrap();
        seen = page
            .items
            .iter()
            .map(|hit| hit.meta.namespace.to_string())
            .filter(|namespace| namespace.starts_with(&root))
            .collect();
        if seen.contains(&shared) {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    assert_eq!(seen, BTreeSet::from([shared]));
}
