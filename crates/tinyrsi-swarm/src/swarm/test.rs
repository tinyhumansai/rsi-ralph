//! Unit tests for request checks, seat specifications and run inputs.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::{ToolKey, load_package};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair")
}

struct Dirs {
    root: tempfile::TempDir,
}

impl Dirs {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("incumbent")).unwrap();
        std::fs::create_dir_all(root.path().join("candidate")).unwrap();
        Self { root }
    }

    fn request(&self) -> SwarmRequest {
        SwarmRequest {
            package: load_package(&fixture()).unwrap(),
            lead: "lead".into(),
            brief: "Do the thing.".into(),
            run_dir: self.root.path().join("run"),
            incumbent: self.root.path().join("incumbent/."),
            candidates: BTreeMap::from([(
                "implementer-a".into(),
                self.root.path().join("candidate"),
            )]),
            model_base_url: Arc::new(|seat| format!("http://gateway/__tag/{seat}/v1")),
            model_api_key: "sk-unit-secret".into(),
            default_model: "default-model".into(),
            temperature: Some(0.3),
            tools: Vec::new(),
            wall: Duration::from_secs(1),
            turn: Duration::from_secs(1),
        }
    }
}

#[test]
fn a_valid_request_resolves_its_checkouts_to_absolute_directories() {
    let dirs = Dirs::new();
    let mut request = dirs.request();
    let hive = check(&mut request).unwrap();
    assert_eq!(hive.seats.len(), 2);
    let incumbent = std::fs::canonicalize(dirs.root.path().join("incumbent")).unwrap();
    assert_eq!(request.incumbent, incumbent);
    assert!(request.candidates["implementer-a"].is_absolute());
}

#[test]
fn a_seat_named_like_the_runtime_is_reserved() {
    let dirs = Dirs::new();
    let mut request = dirs.request();
    for (path, text) in &mut request.package {
        if path == "hive.json" || path == "seats/lead.md" {
            *text = text.replace("\"lead\"", "\"runtime\"");
        }
    }
    request.lead = RUNTIME_SEAT.into();
    assert!(matches!(check(&mut request), Err(Error::ReservedSeat(seat)) if seat == RUNTIME_SEAT));
}

#[test]
fn an_editing_seat_without_a_candidate_is_refused() {
    let dirs = Dirs::new();
    let mut request = dirs.request();
    request.candidates.clear();
    assert!(matches!(check(&mut request), Err(Error::MissingCandidate(seat)) if seat == "implementer-a"));
}

#[test]
fn a_candidate_for_a_read_only_seat_is_refused() {
    let dirs = Dirs::new();
    let mut request = dirs.request();
    request
        .candidates
        .insert("lead".into(), dirs.root.path().join("candidate"));
    assert!(matches!(check(&mut request), Err(Error::UnexpectedCandidate(seat)) if seat == "lead"));
}

#[test]
fn a_checkout_that_is_missing_or_a_file_is_refused() {
    let dirs = Dirs::new();
    let mut missing = dirs.request();
    missing.incumbent = dirs.root.path().join("absent");
    assert!(matches!(check(&mut missing), Err(Error::Checkout { path, .. }) if path.ends_with("absent")));
    let file = dirs.root.path().join("file");
    std::fs::write(&file, "x").unwrap();
    let mut not_dir = dirs.request();
    not_dir.candidates.insert("implementer-a".into(), file.clone());
    assert!(matches!(check(&mut not_dir), Err(Error::Checkout { path, .. }) if path == file));
}

#[test]
fn an_unsupported_tool_key_is_refused_before_the_run() {
    let dirs = Dirs::new();
    let mut request = dirs.request();
    request.tools.push(ToolKey {
        capability: "telepathy".into(),
        provider: "x".into(),
        secret: "s".into(),
        seats: None,
    });
    assert!(matches!(check(&mut request), Err(Error::UnsupportedTool { .. })));
}

#[test]
fn seats_act_in_their_candidate_or_the_incumbent_with_their_model() {
    let dirs = Dirs::new();
    let mut request = dirs.request();
    let hive = check(&mut request).unwrap();
    let context = &hive.lowered.context;
    let (_, lead) = seat::spec(&hive.seats[0], &request, context);
    assert_eq!(lead.id, "lead");
    assert_eq!(lead.model, "fixture-lead");
    assert_eq!(lead.action_dir, request.incumbent);
    assert!(!lead.editing);
    let (_, implementer) = seat::spec(&hive.seats[1], &request, context);
    assert_eq!(implementer.model, "default-model");
    assert_eq!(implementer.action_dir, request.candidates["implementer-a"]);
    assert!(implementer.editing);
    assert_eq!(implementer.role, "implementer");
}

#[test]
fn the_kickoff_lists_every_candidate_after_the_brief() {
    let dirs = Dirs::new();
    let request = dirs.request();
    let body = run::kickoff(&request);
    assert!(body.starts_with("Do the thing.\n\nCandidate checkout for implementer-a: "));
    assert!(body.ends_with("candidate"));
}

#[test]
fn the_runtime_config_starts_nothing_on_its_own() {
    let config = run::runtime_config();
    assert_eq!(config.agent.tool_dispatcher, "native");
    assert!(!config.local_ai.runtime_enabled);
    assert!(!config.memory.recall.enabled);
    assert!(!config.memory.conversations.enabled);
}

#[test]
fn the_seat_tool_lists_keep_the_hive_tools_and_deny_waiting() {
    assert!(seat::HIVEMIND_TOOLS.contains(&"hivemind_send_agent"));
    assert_eq!(seat::DENIED_TOOLS, ["wait", "wait_loop"]);
}

#[test]
fn debug_output_hides_the_model_key_and_usage_starts_complete() {
    let dirs = Dirs::new();
    let shown = format!("{:?}", dirs.request());
    assert!(shown.contains("default-model"));
    assert!(!shown.contains("sk-unit-secret"));
    let usage = SeatUsage::default();
    assert_eq!(usage.turns, 0);
    assert!(usage.cost_complete);
}
