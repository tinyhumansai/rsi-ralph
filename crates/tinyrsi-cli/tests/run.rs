//! `tinyrsi run` end to end, offline: the graph-colouring loop folder, the
//! real gateway, swarm, scorer and engine, against a scripted
//! OpenAI-compatible model standing in for `OpenRouter`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use clap::Parser;
use serde_json::{Value, json};
use tinyrsi_cli::cli::{Cli, execute, exit_code};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::method};

const KEY: &str = "sk-or-fixture-key-0000";

fn completion(message: &Value) -> Value {
    let finish = if message.get("tool_calls").is_some() {
        "tool_calls"
    } else {
        "stop"
    };
    json!({
        "id": "fixture", "object": "chat.completion", "created": 0, "model": "z-ai/glm-5.3-flash",
        "choices": [{"index": 0, "message": message, "finish_reason": finish}],
        "usage": {"prompt_tokens": 100, "completion_tokens": 20, "total_tokens": 120}
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

/// Scripts each seat by its system prompt: the lead assigns once, the
/// implementer rewrites `coloring.py` with DSATUR once, then both just talk.
struct Lab {
    calls: Mutex<BTreeMap<&'static str, usize>>,
    dsatur: String,
}

impl Respond for Lab {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body = String::from_utf8_lossy(&request.body);
        let seat = if body.contains("You lead a two-seat lab") {
            "lead"
        } else if body.contains("You implement one assignment") {
            "implementer"
        } else {
            "other"
        };
        let call = {
            let mut calls = self.calls.lock().unwrap();
            let n = calls.entry(seat).or_default();
            *n += 1;
            *n
        };
        let reply = match (seat, call) {
            ("lead", 1) => tool_call(
                "lead-1",
                "hivemind_send_agent",
                &json!({"agent_id": "implementer", "message_id": "assign-1",
                    "body": "Rewrite coloring.py to use DSATUR."}),
            ),
            ("implementer", 1) => tool_call(
                "impl-1",
                "file_write",
                &json!({"path": "coloring.py", "content": self.dsatur}),
            ),
            ("lead", _) => text("Assigned and reviewed; done."),
            ("implementer", _) => text("Rewrote coloring.py with DSATUR."),
            _ => text("ok"),
        };
        if body.contains("\"stream\":true") {
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(stream(&reply))
        } else {
            ResponseTemplate::new(200).set_body_json(reply)
        }
    }
}

/// The same completion as OpenAI-style server-sent events.
fn stream(reply: &Value) -> String {
    let message = &reply["choices"][0]["message"];
    let mut delta = json!({"role": "assistant"});
    if let Some(content) = message["content"].as_str() {
        delta["content"] = json!(content);
    }
    if let Some(calls) = message["tool_calls"].as_array() {
        let calls: Vec<Value> = calls
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut c = c.clone();
                c["index"] = json!(i);
                c
            })
            .collect();
        delta["tool_calls"] = json!(calls);
    }
    let chunk = |choice: Value, usage: Option<&Value>| {
        let mut body = json!({"id": "fixture", "object": "chat.completion.chunk", "created": 0,
            "model": "z-ai/glm-5.3-flash", "choices": [choice]});
        if let Some(usage) = usage {
            body["usage"] = usage.clone();
        }
        format!("data: {body}\n\n")
    };
    let finish = reply["choices"][0]["finish_reason"].clone();
    format!(
        "{}{}data: [DONE]\n\n",
        chunk(
            json!({"index": 0, "delta": delta, "finish_reason": null}),
            None
        ),
        chunk(
            json!({"index": 0, "delta": {}, "finish_reason": finish}),
            Some(&reply["usage"])
        )
    )
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[test]
fn a_loop_folder_runs_end_to_end_through_the_binary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tmp = tempfile::tempdir().unwrap();
    let loop_dir = tmp.path().join("graph-coloring");
    copy(&root.join("loops/graph-coloring"), &loop_dir);
    let home = tmp.path().join("home");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(Lab {
                calls: Mutex::new(BTreeMap::new()),
                dsatur: std::fs::read_to_string(
                    root.join("examples/graph_coloring/proposals/dsatur.py"),
                )
                .unwrap(),
            })
            .mount(&server)
            .await;
        server
    });
    let upstream = server.uri();
    let env: Arc<BTreeMap<&str, String>> = Arc::new(BTreeMap::from([
        ("OPENROUTER_API_KEY", KEY.to_owned()),
        ("TINYRSI_UPSTREAM", upstream),
        ("PATH", std::env::var("PATH").unwrap()),
        ("HOME", std::env::var("HOME").unwrap()),
    ]));
    let lookup = move |name: &str| env.get(name).cloned();

    let cli = Cli::parse_from([
        "tinyrsi",
        "--home",
        home.to_str().unwrap(),
        "run",
        loop_dir.to_str().unwrap(),
        "--run",
        "e2e",
        "--epochs",
        "1",
        "--port",
        "0",
    ]);
    let mut out = Vec::new();
    let result = execute(cli, &lookup, &mut out);
    let printed = String::from_utf8(out).unwrap();
    assert!(result.is_ok(), "{result:?}\n{printed}");
    assert_eq!(exit_code(&result), 0);
    assert!(printed.contains("ui: http://127.0.0.1:"), "{printed}");
    assert!(printed.contains("after 1 promotions"), "{printed}");

    let run = home.join("runs/graph-coloring/e2e");
    let record: Value =
        serde_json::from_str(&std::fs::read_to_string(run.join("run.json")).unwrap()).unwrap();
    assert_eq!(record["status"], "finished");
    let ledger: Value =
        serde_json::from_str(&std::fs::read_to_string(run.join("ledger.json")).unwrap()).unwrap();
    assert_eq!(ledger["candidates"][0]["status"], "promoted");
    assert_eq!(ledger["candidates"][0]["changed"], json!(["coloring.py"]));
    // First fit uses 17 colours on these graphs; DSATUR uses fewer.
    let baseline = ledger["epochs"][0]["incumbent"].as_str().unwrap();
    assert_ne!(record["incumbent"].as_str().unwrap(), baseline);
    assert!(
        record["incumbent_score"].as_f64().unwrap() < 17.0,
        "{record}"
    );
    assert!(
        std::fs::read_to_string(run.join("incumbent/coloring.py"))
            .unwrap()
            .contains("DSATUR")
    );
    assert!(record["spend_usd"].as_f64().unwrap() > 0.0, "{record}");
    assert!(run.join("epoch-1/hive/events.jsonl").exists());

    for file in files(&run).into_iter().chain(files(&home)) {
        if let Ok(text) = std::fs::read_to_string(&file) {
            assert!(!text.contains(KEY), "{} holds the key", file.display());
        }
    }

    // The stored run is listed and summarized.
    let mut out = Vec::new();
    let cli = Cli::parse_from(["tinyrsi", "--home", home.to_str().unwrap(), "runs"]);
    execute(cli, &lookup, &mut out).unwrap();
    assert!(
        String::from_utf8(out)
            .unwrap()
            .starts_with("graph-coloring/e2e\tfinished")
    );
}

#[test]
fn the_binary_reports_success_and_input_errors_by_exit_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let home = tempfile::tempdir().unwrap();
    let tinyrsi = |args: &[&str], key: bool| {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_tinyrsi"));
        command.arg("--home").arg(home.path()).args(args).env_remove("OPENROUTER_API_KEY");
        if key {
            command.env("OPENROUTER_API_KEY", KEY);
        }
        command.output().unwrap()
    };
    let loop_dir = root.join("loops/graph-coloring");
    let ok = tinyrsi(&["check", loop_dir.to_str().unwrap()], true);
    assert_eq!(ok.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&ok.stdout).starts_with("ok: graph-coloring"));
    let missing = tinyrsi(&["check", loop_dir.to_str().unwrap()], false);
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing keys: OPENROUTER_API_KEY"));
}
