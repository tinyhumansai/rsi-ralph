//! Unit tests for a seat's executable tools.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{belt, names, validate};
use crate::Error;
use crate::load_package;
use crate::package::Hive;

/// The pair fixture with `command` declared as the tool `note`, given to the
/// implementer through a toolpack.
fn hive_with(command: &[&str]) -> Hive {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair");
    let mut files: BTreeMap<String, String> = load_package(&fixture).unwrap();
    let mut manifest: Value = serde_json::from_str(&files["hive.json"]).unwrap();
    manifest["tools"] = json!([{
        "id": "note",
        "description": "Writes its arguments to note.json",
        "command": command,
        "args": {"type": "object", "properties": {"text": {"type": "string"}}},
        "side_effects": "write"
    }]);
    manifest["toolpacks"] = json!([{"id": "notes", "tools": ["note"]}]);
    manifest["seats"][1]["toolpacks"] = json!(["notes"]);
    files.insert("hive.json".into(), manifest.to_string());
    // A caller building the map by hand can ship a package program;
    // `load_package` never reads one.
    files.insert("tools/note.py".into(), "print('note')\n".into());
    Hive::open(&files).unwrap()
}

fn plain() -> Hive {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair");
    Hive::open(&load_package(&fixture).unwrap()).unwrap()
}

fn implementer(hive: &Hive) -> &crate::package::SeatPlan {
    hive.seats
        .iter()
        .find(|plan| plan.seat.id == "implementer-a")
        .unwrap()
}

#[test]
fn a_seat_lists_the_programs_its_toolpack_gives_it() {
    let hive = hive_with(&["sh", "-c", "cat > note.json"]);
    assert_eq!(names(implementer(&hive)).collect::<Vec<_>>(), ["note"]);
    let lead = hive.seats.iter().find(|plan| plan.seat.id == "lead").unwrap();
    assert_eq!(names(lead).count(), 0);
}

#[test]
fn programs_on_path_are_accepted() {
    validate(&hive_with(&["sh", "-c", "cat > note.json"])).unwrap();
    validate(&plain()).unwrap();
}

#[test]
fn a_package_file_program_is_refused() {
    let error = validate(&hive_with(&["python3", "tools/note.py"])).unwrap_err();
    assert!(
        matches!(&error, Error::PackageProgram { seat, tool } if seat == "implementer-a" && tool == "note"),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "seat implementer-a declares tool note, whose program is a package file"
    );
}

#[test]
fn without_programs_there_is_no_belt() {
    assert!(belt(&plain(), Path::new("/run"), |_| PathBuf::from("/work")).is_none());
}

#[test]
fn the_belt_gives_each_seat_only_its_own_programs() {
    let hive = hive_with(&["sh", "-c", "cat > note.json"]);
    let source = belt(&hive, Path::new("/run"), |_| PathBuf::from("/work")).unwrap();
    let own = source(openhuman_embed::TurnContext::new("implementer-a", None));
    let names: Vec<&str> = own.tools.iter().map(|tool| tool.name()).collect();
    assert_eq!(names, ["note"]);
    assert!(own.visible.contains("note"));
    let other = source(openhuman_embed::TurnContext::new("lead", None));
    assert!(other.tools.is_empty());
}
