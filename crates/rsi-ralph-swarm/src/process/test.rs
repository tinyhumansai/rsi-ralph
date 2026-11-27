//! Process seats: which seats run on a process, what they are refused,
//! what their harness receives and the host tools they get.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tinyhivemind_hives::Coordinator;
use tinyhivemind_lang::{HarnessKind, ProviderKind};

use super::*;
use crate::{ToolKey, load_package};

/// The `pair` fixture, its `implementer-a` on `kind` and its lead in-process.
fn request(kind: HarnessKind, root: &Path) -> SwarmRequest {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair");
    std::fs::create_dir_all(root.join("incumbent")).unwrap();
    std::fs::create_dir_all(root.join("candidate")).unwrap();
    SwarmRequest {
        package: load_package(&fixture).unwrap(),
        lead: "lead".into(),
        brief: "Do the thing.".into(),
        run_dir: root.join("run"),
        incumbent: root.join("incumbent"),
        candidates: BTreeMap::from([("implementer-a".into(), root.join("candidate"))]),
        model_base_url: Arc::new(|seat| format!("http://gateway/__tag/swarm/{seat}/v1")),
        model_api_key: "gateway".into(),
        default_model: "default-model".into(),
        temperature: None,
        tools: Vec::new(),
        memory: None,
        wall: std::time::Duration::from_secs(1),
        turn: std::time::Duration::from_secs(1),
        seat_harnesses: BTreeMap::from([("implementer-a".into(), kind)]),
        harnesses: HarnessInstalls::default(),
    }
}

fn plan<'a>(hive: &'a Hive, id: &str) -> &'a SeatPlan {
    hive.seats.iter().find(|plan| plan.seat.id == id).unwrap()
}

#[test]
fn a_seat_naming_a_harness_runs_on_a_process_and_the_rest_in_process() {
    let root = tempfile::tempdir().unwrap();
    let request = request(HarnessKind::Codex, root.path());
    let hive = Hive::open(&request.package).unwrap();
    assert!(is_process(plan(&hive, "implementer-a"), &request));
    assert!(!is_process(plan(&hive, "lead"), &request));
    assert!(seat(plan(&hive, "lead"), &request, &hive).is_none());
    validate(&hive, &request).unwrap();
}

#[test]
fn a_codex_seat_gets_a_clean_seat_its_model_through_the_gateway_and_tools() {
    let root = tempfile::tempdir().unwrap();
    let request = request(HarnessKind::Codex, root.path());
    let hive = Hive::open(&request.package).unwrap();
    let (process_seat, info) = seat(plan(&hive, "implementer-a"), &request, &hive).unwrap();
    let lowered = &process_seat.seat;
    // The swarm keeps the package's own settings; the harness gets none.
    assert_eq!(lowered.config, serde_json::Value::Null);
    assert_eq!(lowered.integration.tools.allow, Vec::<String>::new());
    assert!(
        lowered.prompt.contains("## Your working directory"),
        "{}",
        lowered.prompt
    );
    assert!(lowered.prompt.contains(crate::HIVE_RULES));
    let model = lowered.integration.model.as_ref().unwrap();
    assert_eq!(model.model.name, "default-model");
    let provider = model.provider.as_ref().unwrap();
    assert_eq!(provider.kind, ProviderKind::Openai);
    assert_eq!(
        provider.endpoint.as_deref(),
        Some("http://gateway/__tag/swarm/implementer-a/v1")
    );
    assert!(provider.credential.is_none());
    assert_eq!(
        lowered.integration.harness.as_ref().map(|h| h.kind),
        Some(HarnessKind::Codex)
    );
    assert_eq!(
        process_seat.directory,
        request
            .run_dir
            .join(HARNESS_DIR)
            .join("seats/implementer-a")
    );
    assert_eq!(info.harness.as_deref(), Some("codex"));
    assert!(info.editing);
    assert_eq!(info.action_dir, root.path().join("candidate"));
    assert!(
        format!("{:?}", process_seat.tools).contains("write_file"),
        "an editing seat allowed file_write gets write_file"
    );
}

#[test]
fn a_claude_code_seat_takes_the_api_root_and_anthropic() {
    let root = tempfile::tempdir().unwrap();
    let request = request(HarnessKind::ClaudeCode, root.path());
    let hive = Hive::open(&request.package).unwrap();
    let (process_seat, info) = seat(plan(&hive, "implementer-a"), &request, &hive).unwrap();
    let model = process_seat.seat.integration.model.unwrap();
    let provider = model.provider.unwrap();
    assert_eq!(provider.kind, ProviderKind::Anthropic);
    assert_eq!(
        provider.endpoint.as_deref(),
        Some("http://gateway/__tag/swarm/implementer-a")
    );
    assert_eq!(info.harness.as_deref(), Some("claude-code"));
}

#[test]
fn what_only_an_in_process_seat_has_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let mut keyed = request(HarnessKind::Codex, root.path());
    keyed.tools = vec![ToolKey {
        capability: crate::WEB_SEARCH.into(),
        provider: "brave".into(),
        secret: "s".into(),
        seats: None,
    }];
    let hive = Hive::open(&keyed.package).unwrap();
    assert!(matches!(
        validate(&hive, &keyed),
        Err(Error::HarnessUnsupported { ref seat, what: "tool keys" }) if seat == "implementer-a"
    ));
    let mut unknown = request(HarnessKind::Codex, root.path());
    unknown
        .seat_harnesses
        .insert("ghost".into(), HarnessKind::Codex);
    let hive = Hive::open(&unknown.package).unwrap();
    assert!(matches!(
        validate(&hive, &unknown),
        Err(Error::UnknownHarnessSeat(ref seat)) if seat == "ghost"
    ));
    let mut clashing = request(HarnessKind::Codex, root.path());
    let mut manifest: serde_json::Value =
        serde_json::from_str(&clashing.package["hive.json"]).unwrap();
    manifest["tools"] = serde_json::json!([{
        "id": "read_file", "description": "x", "command": ["cat"],
        "args": {"type": "object"}, "side_effects": "read"
    }]);
    manifest["toolpacks"] = serde_json::json!([{"id": "files", "tools": ["read_file"]}]);
    manifest["seats"][1]["toolpacks"] = serde_json::json!(["files"]);
    clashing
        .package
        .insert("hive.json".into(), manifest.to_string());
    let hive = Hive::open(&clashing.package).unwrap();
    assert!(matches!(
        validate(&hive, &clashing),
        Err(Error::ToolNameClash { ref tool, .. }) if tool == "read_file"
    ));
    let mut remembered = request(HarnessKind::Codex, root.path());
    remembered.memory =
        Some(crate::SwarmMemory::cortexdb("http://127.0.0.1:3141", "sk-unit", "team:t").unwrap());
    let hive = Hive::open(&remembered.package).unwrap();
    assert!(matches!(
        validate(&hive, &remembered),
        Err(Error::HarnessUnsupported {
            what: "seat memory",
            ..
        })
    ));
}

#[test]
fn a_process_seat_has_no_known_usage() {
    let usage = unknown_usage();
    assert!(!usage.cost_complete);
    assert_eq!(usage.turns, 0);
}

#[test]
fn the_installs_come_from_the_environment() {
    let env = BTreeMap::from([
        (NODE_ENV, "/opt/node"),
        (CODEX_ACP_ENV, "/opt/codex-acp.js"),
        (CODEX_ENV, "/opt/codex"),
        (CLAUDE_CODE_ENV, ""),
    ]);
    let found = harness_installs(&|name| env.get(name).map(|v| (*v).to_owned()));
    assert_eq!(found.codex, Some(PathBuf::from("/opt/codex")));
    assert_eq!(found.claude_code, None);
    let installs = found.pinned();
    assert_eq!(installs.node, Some(PathBuf::from("/opt/node")));
    let codex = installs.codex.unwrap();
    assert_eq!(codex.adapter.version, CODEX_ACP_VERSION);
    assert_eq!(codex.binary, PathBuf::from("/opt/codex"));
    // Unset or empty is not installed.
    assert!(installs.claude_code.is_none());
    assert!(installs.openhuman.is_none());
    assert!(installs.openclaw.is_none());
    let every = BTreeMap::from([
        (OPENHUMAN_ENV, "/opt/openhuman-core"),
        (CLAUDE_CODE_ENV, "/opt/claude-acp.js"),
        (OPENCLAW_ENV, "/opt/openclaw.mjs"),
    ]);
    let installs = harness_installs(&|name| every.get(name).map(|v| (*v).to_owned())).pinned();
    assert_eq!(installs.openhuman.unwrap().version, OPENHUMAN_VERSION);
    assert_eq!(installs.claude_code.unwrap().version, CLAUDE_CODE_VERSION);
    assert_eq!(installs.openclaw.unwrap().version, OPENCLAW_VERSION);
    // Codex needs both its adapter and its binary.
    assert!(installs.codex.is_none());
    let none = harness_installs(&|_| None);
    assert_eq!(none, HarnessInstalls::default());
    assert_eq!(none.pinned(), tinyhivemind_deploy::Installs::default());
}

#[test]
fn host_tools_follow_the_allow_list_access_and_deny_list() {
    let ids = |tools: Vec<&tools::Standard>| tools.iter().map(|t| t.id).collect::<Vec<_>>();
    let names = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
    // No list: everything the access allows.
    assert_eq!(
        ids(tools::granted(&[], &[], true)),
        [
            "read_file",
            "list_files",
            "search_files",
            "write_file",
            "edit_file",
            "shell"
        ]
    );
    assert_eq!(
        ids(tools::granted(&[], &[], false)),
        ["read_file", "list_files", "search_files"]
    );
    // OpenHuman's names grant the host tools standing in for them.
    let allow = names(&["file_read", "glob", "edit", "shell"]);
    assert_eq!(
        ids(tools::granted(&allow, &[], true)),
        ["read_file", "list_files", "edit_file", "shell"]
    );
    // A read-only seat never gets a writing tool, whatever it is allowed.
    assert_eq!(
        ids(tools::granted(&allow, &[], false)),
        ["read_file", "list_files"]
    );
    assert_eq!(
        ids(tools::granted(&allow, &names(&["shell"]), true)),
        ["read_file", "list_files", "edit_file"]
    );
}

#[test]
fn every_host_tool_declares_a_valid_executable_tool() {
    for tool in &tools::STANDARD {
        let declaration = tools::declaration(tool);
        assert_eq!(declaration.id, tool.id);
        assert_ne!(declaration.description, "");
        let writes = declaration.side_effects == tinyhivemind_lang::SideEffects::Write;
        assert_eq!(writes, tool.writes, "{}", tool.id);
    }
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

async fn coordinator(dir: &Path) -> Coordinator {
    Coordinator::new(
        "runtime".into(),
        std::sync::Arc::new(tinyhivemind_hives::SqliteStorage::open(dir.join("hive.db")).unwrap()),
        tinyhivemind_hives::CoordinatorOptions::default(),
    )
    .await
    .unwrap()
}

#[test]
fn no_process_seats_start_nothing_and_stop_cleanly() {
    let root = tempfile::tempdir().unwrap();
    let request = request(HarnessKind::Codex, root.path());
    block_on(async {
        let coordinator = coordinator(root.path()).await;
        let started = start(&coordinator, Vec::new(), &request, &request.run_dir).await;
        assert!(started.unwrap().is_none());
        stop(None).await.unwrap();
    });
    assert!(!request.run_dir.join(HARNESS_DIR).exists());
}

#[test]
fn a_seat_whose_harness_is_not_installed_starts_nothing() {
    let root = tempfile::tempdir().unwrap();
    let request = request(HarnessKind::Codex, root.path());
    let hive = Hive::open(&request.package).unwrap();
    let (process_seat, _) = seat(plan(&hive, "implementer-a"), &request, &hive).unwrap();
    block_on(async {
        let coordinator = coordinator(root.path()).await;
        let started = start(&coordinator, vec![process_seat], &request, &request.run_dir).await;
        assert!(
            matches!(started, Err(Error::Harness(ref source)) if source.to_string().contains("codex")),
            "{:?}",
            started.map(|seats| seats.is_some())
        );
        assert_eq!(coordinator.list_agents().unwrap(), Vec::<String>::new());
    });
}
