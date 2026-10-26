//! Unit tests for the seat runtime's own tool names.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use super::{names, validate};
use crate::package::Hive;
use crate::{Error, check_package, load_package};

/// The pair fixture with an executable tool `tool` given to the implementer.
fn files_with(tool: &str) -> BTreeMap<String, String> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair");
    let mut files = load_package(&fixture).unwrap();
    let mut manifest: Value = serde_json::from_str(&files["hive.json"]).unwrap();
    manifest["tools"] = json!([{
        "id": tool,
        "description": "Looks something up",
        "command": ["echo", "hi"],
        "args": {"type": "object", "properties": {}},
        "side_effects": "read"
    }]);
    manifest["toolpacks"] = json!([{"id": "extras", "tools": [tool]}]);
    manifest["seats"][1]["toolpacks"] = json!(["extras"]);
    files.insert("hive.json".into(), manifest.to_string());
    files
}

#[test]
fn the_runtime_lists_the_tools_it_registers_itself() {
    let taken = names();
    for tool in ["web_search", "web_fetch", "file_read", "shell", "web_search_tool"] {
        assert!(taken.contains(tool), "{tool} missing from {taken:?}");
    }
    assert!(!taken.contains("lookup_notes"));
}

#[test]
fn a_tool_named_like_a_builtin_is_refused_naming_seat_and_tool() {
    let hive = Hive::open(&files_with("web_search")).unwrap();
    let error = validate(&hive).unwrap_err();
    assert!(
        matches!(&error, Error::ToolNameClash { seat, tool } if seat == "implementer-a" && tool == "web_search"),
        "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("\"web_search\" is already registered"), "{message}");
}

#[test]
fn a_uniquely_named_tool_is_accepted() {
    validate(&Hive::open(&files_with("lookup_notes")).unwrap()).unwrap();
}

#[test]
fn check_package_applies_the_same_rule_to_a_package_map() {
    check_package(&files_with("lookup_notes")).unwrap();
    assert!(matches!(
        check_package(&files_with("web_fetch")),
        Err(Error::ToolNameClash { .. })
    ));
    assert!(matches!(
        check_package(&BTreeMap::new()),
        Err(Error::Package(_))
    ));
}

#[test]
fn two_hive_tools_with_one_id_are_refused_by_the_language() {
    let mut files = files_with("lookup_notes");
    let mut manifest: Value = serde_json::from_str(&files["hive.json"]).unwrap();
    let tool = manifest["tools"][0].clone();
    manifest["tools"].as_array_mut().unwrap().push(tool);
    files.insert("hive.json".into(), manifest.to_string());
    assert!(matches!(check_package(&files), Err(Error::Package(_))));
}
