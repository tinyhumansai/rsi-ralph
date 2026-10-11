//! Building the hive proposer from a loop: editing seats and tool keys.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::manifest;
use std::path::Path;

fn example(edit: impl FnOnce(&mut serde_json::Value)) -> Loop {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../loops/graph-coloring");
    let mut files = manifest::load_dir(&dir).unwrap();
    let mut m: serde_json::Value = serde_json::from_str(&files["rsi.json"]).unwrap();
    edit(&mut m);
    files.insert("rsi.json".into(), m.to_string());
    manifest::parse(&files).unwrap()
}

fn keys(parsed: &Loop) -> Keys {
    let dir = tempfile::tempdir().unwrap();
    Keys::resolve(parsed, dir.path(), &|_| Some("sk-value".into())).unwrap()
}

#[test]
fn takes_editing_seats_models_and_tool_keys_from_the_loop() {
    let parsed = example(|m| {
        m["keys"] = serde_json::json!({ "web_search": { "provider": "gemini", "env": "GEMINI_API_KEY", "seats": ["lead"] } });
    });
    let hive = Hive::new(&parsed, &keys(&parsed), "http://127.0.0.1:9/").unwrap();
    assert_eq!(hive.editing_seats(), vec!["implementer"]);
    assert_eq!(hive.default_model, manifest::DEFAULT_MODEL);
    assert_eq!(hive.temperature, Some(0.4));
    assert_eq!(hive.tools.len(), 1);
    assert_eq!(hive.tools[0].capability, "web_search");
    assert_eq!(hive.base_url, "http://127.0.0.1:9");
    let shown = format!("{hive:?}");
    assert!(shown.contains("implementer") && !shown.contains("sk-value"));
}

#[test]
fn a_tool_key_without_a_value_is_missing() {
    let parsed = example(|m| {
        m["keys"] = serde_json::json!({ "web_search": { "provider": "gemini", "env": "GEMINI_API_KEY" } });
    });
    assert!(matches!(
        Hive::new(&parsed, &Keys::default(), "http://x"),
        Err(Error::MissingKeys { names }) if names == ["GEMINI_API_KEY"]
    ));
}

#[test]
fn a_hive_with_no_editing_seat_is_refused() {
    let mut parsed = example(|_| {});
    let hive_json = parsed.hive.get_mut("hive.json").unwrap();
    *hive_json = hive_json.replace("{candidate}", "{incumbent}");
    assert!(matches!(
        Hive::new(&parsed, &keys(&parsed), "http://x"),
        Err(Error::Proposer(reason)) if reason.contains("no seat edits")
    ));
}
