//! `rsi-ralph run` end to end, offline: the graph-colouring loop folder, the
//! real gateway, swarm, scorer and engine, against a scripted
//! OpenAI-compatible model standing in for `OpenRouter`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use clap::Parser;
use rsi_ralph_cli::cli::{Cli, execute, exit_code};
use serde_json::{Value, json};
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
        let parsed: Value = serde_json::from_slice(&request.body).unwrap_or_default();
        let forced = parsed["tool_choice"]["function"]["name"]
            .as_str()
            .unwrap_or("");
        if forced == "grade" {
            let reply = tool_call(
                "judge-1",
                "grade",
                &json!({"score": 8, "rationale": "a clear change"}),
            );
            return ResponseTemplate::new(200).set_body_json(reply);
        }
        if forced == "submit_items" {
            let items = json!({"items": [{"idea": "colour the most constrained vertex first", "why": "saturation"}]});
            return ResponseTemplate::new(200).set_body_json(tool_call(
                "ideas-1",
                "submit_items",
                &items,
            ));
        }
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

/// Copy the example loop, adding a judge on a priced extra model and a small
/// creative retreat.
fn prepare(root: &Path, loop_dir: &Path) {
    copy(&root.join("loops/graph-coloring"), loop_dir);
    // Add a judge on a priced extra model and a small creative retreat.
    let manifest = loop_dir.join("rsi.json");
    let mut m: Value = serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    m["models"] = json!({"judge": "openai/gpt-4.1-mini"});
    m["budget"]["prices"] = json!({"openai/gpt-4.1-mini":
        {"prompt_per_mtok": 0.4, "completion_per_mtok": 1.6, "provider": "openai"}});
    m["score"]["scorers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "clarity", "judge": "scorers/clarity.md", "weight": 0.2, "raters": 1}));
    m["creativity"]["retreat"] =
        json!({"variations": 1, "turns": 2, "ideas": 1, "mode": "brainstorm"});
    std::fs::write(&manifest, m.to_string()).unwrap();
    std::fs::write(
        loop_dir.join("scorers/clarity.md"),
        "Is the change clear and general?",
    )
    .unwrap();
}

/// The scripted model server, with the runtime that keeps it serving.
fn lab(root: &Path) -> (tokio::runtime::Runtime, MockServer) {
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
    (runtime, server)
}

#[test]
fn a_loop_folder_runs_end_to_end_through_the_binary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tmp = tempfile::tempdir().unwrap();
    let loop_dir = tmp.path().join("graph-coloring");
    prepare(&root, &loop_dir);
    let home = tmp.path().join("home");

    let (_runtime, server) = lab(&root);
    let upstream = server.uri();
    let env: Arc<BTreeMap<&str, String>> = Arc::new(BTreeMap::from([
        ("OPENROUTER_API_KEY", KEY.to_owned()),
        ("TINYRSI_UPSTREAM", upstream),
        ("PATH", std::env::var("PATH").unwrap()),
        ("HOME", std::env::var("HOME").unwrap()),
    ]));
    let lookup = move |name: &str| env.get(name).cloned();

    let cli = Cli::parse_from([
        "rsi-ralph",
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
    let brief = std::fs::read_to_string(run.join("epoch-1/brief.md")).unwrap();
    assert!(
        brief.contains("colour the most constrained vertex first"),
        "{brief}"
    );
    let judged = &ledger["candidates"][0]["scorecard"]["judges"]["clarity"];
    assert!(
        (judged["quality"].as_f64().unwrap() - 0.8).abs() < 1e-9,
        "{judged}"
    );
    assert!(home.join("retreat/graph-coloring/ideas.json").exists());

    for file in files(&run).into_iter().chain(files(&home)) {
        if let Ok(text) = std::fs::read_to_string(&file) {
            assert!(!text.contains(KEY), "{} holds the key", file.display());
        }
    }

    // The stored run is listed and summarized.
    let mut out = Vec::new();
    let cli = Cli::parse_from(["rsi-ralph", "--home", home.to_str().unwrap(), "runs"]);
    execute(cli, &lookup, &mut out).unwrap();
    assert!(
        String::from_utf8(out)
            .unwrap()
            .starts_with("graph-coloring/e2e\tfinished")
    );

    // The first run of the loop records that it had nothing to compare with.
    let log = std::fs::read_to_string(run.join("upgrades.jsonl")).unwrap();
    let applied: Value = serde_json::from_str(log.lines().next().unwrap()).unwrap();
    assert_eq!(applied["kind"], "applied");
    assert_eq!(applied["run"], "e2e");
    assert!(applied.get("previous_run").is_none(), "{applied}");
    assert!(
        std::fs::read_to_string(run.join("upgrades.md"))
            .unwrap()
            .contains("## Run e2e started")
    );

    // A seat prompt edited with no upgrade record is flagged by the next run.
    let seat = loop_dir.join("hive/seats/lead.md");
    let mut prompt = std::fs::read_to_string(&seat).unwrap();
    prompt.push_str("\nAssign every seat before reading anything.\n");
    std::fs::write(&seat, prompt).unwrap();
    let cli = Cli::parse_from([
        "rsi-ralph",
        "--home",
        home.to_str().unwrap(),
        "run",
        loop_dir.to_str().unwrap(),
        "--run",
        "e2e-2",
        "--epochs",
        "1",
        "--no-serve",
    ]);
    let mut out = Vec::new();
    let result = execute(cli, &lookup, &mut out);
    let printed = String::from_utf8(out).unwrap();
    assert!(result.is_ok(), "{result:?}\n{printed}");
    let second = home.join("runs/graph-coloring/e2e-2");
    let record: Value =
        serde_json::from_str(&std::fs::read_to_string(second.join("run.json")).unwrap()).unwrap();
    assert_eq!(
        record["warnings"][0],
        "changed since run e2e with no upgrade record: hive/seats/lead.md"
    );
    let log = std::fs::read_to_string(second.join("upgrades.jsonl")).unwrap();
    let entries: Vec<Value> = log
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(entries[0]["previous_run"], "e2e");
    assert_eq!(entries[0]["unexplained"], json!(["hive/seats/lead.md"]));
    assert_eq!(entries[1]["contributors"][0]["author"]["kind"], "loop");
}

#[test]
fn the_binary_reports_success_and_input_errors_by_exit_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let home = tempfile::tempdir().unwrap();
    let ralph = |args: &[&str], key: bool| {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_rsi-ralph"));
        command
            .arg("--home")
            .arg(home.path())
            .args(args)
            .env_remove("OPENROUTER_API_KEY");
        if key {
            command.env("OPENROUTER_API_KEY", KEY);
        }
        command.output().unwrap()
    };
    let loop_dir = root.join("loops/graph-coloring");
    let ok = ralph(&["check", loop_dir.to_str().unwrap()], true);
    assert_eq!(ok.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&ok.stdout).starts_with("ok: graph-coloring"));
    let missing = ralph(&["check", loop_dir.to_str().unwrap()], false);
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing keys: OPENROUTER_API_KEY"));
}

#[test]
fn a_provider_refusing_every_call_fails_the_run_with_its_message() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tmp = tempfile::tempdir().unwrap();
    let loop_dir = tmp.path().join("graph-coloring");
    copy(&root.join("loops/graph-coloring"), &loop_dir);
    let home = tmp.path().join("home");

    // A key over its daily limit: OpenRouter answers every call with 403.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({"error": {
                "message": "Key limit exceeded (daily limit).", "code": 403}})))
            .mount(&server)
            .await;
        server
    });
    let env: Arc<BTreeMap<&str, String>> = Arc::new(BTreeMap::from([
        ("OPENROUTER_API_KEY", KEY.to_owned()),
        ("TINYRSI_UPSTREAM", server.uri()),
        ("PATH", std::env::var("PATH").unwrap()),
        ("HOME", std::env::var("HOME").unwrap()),
    ]));
    let lookup = move |name: &str| env.get(name).cloned();
    let cli = Cli::parse_from([
        "rsi-ralph",
        "--home",
        home.to_str().unwrap(),
        "run",
        loop_dir.to_str().unwrap(),
        "--run",
        "refused",
        "--epochs",
        "3",
        "--port",
        "0",
    ]);
    let mut out = Vec::new();
    let result = execute(cli, &lookup, &mut out);
    let printed = String::from_utf8(out).unwrap();
    assert_eq!(exit_code(&result), 1, "{result:?}\n{printed}");
    let message = result.unwrap_err().to_string();
    assert!(
        message.starts_with("provider refused ")
            && message.contains("model calls (403: Key limit exceeded (daily limit).)"),
        "{message}"
    );
    assert!(printed.contains("stopped: provider_refused"), "{printed}");

    let record: Value = serde_json::from_str(
        &std::fs::read_to_string(home.join("runs/graph-coloring/refused/run.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(record["status"], "failed");
    assert_eq!(record["stop_reason"], "provider_refused");
    assert_eq!(record["epochs"], 1);
    assert_eq!(record["error"].as_str().unwrap(), message);
}
