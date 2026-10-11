//! Unit tests for reading hive packages and lowering their seats.
#![allow(clippy::unwrap_used)]

use super::*;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair")
}

/// A writable copy of the pair fixture.
fn copy() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in load_package(&fixture()).unwrap() {
        let target = dir.path().join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, text).unwrap();
    }
    dir
}

#[test]
fn loads_root_manifests_and_nested_documents_only() {
    let files = load_package(&fixture()).unwrap();
    let keys: Vec<&str> = files.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "constitution.json",
            "context/rules.md",
            "hive.json",
            "memory.json",
            "roles/implementer.md",
            "roles/lead.md",
            "seats/implementer-a.md",
            "seats/lead.md",
        ]
    );
}

#[test]
fn skips_hidden_entries_symlinks_and_stray_files() {
    let dir = copy();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/config.json"), "{").unwrap();
    std::fs::write(root.join("seats/.draft.md"), "draft").unwrap();
    std::fs::write(root.join("seats/ReadMe.MD"), "index").unwrap();
    std::fs::write(root.join("notes.md"), "root markdown").unwrap();
    std::fs::write(root.join("seats/extra.json"), "{").unwrap();
    std::fs::write(root.join("seats/script.sh"), "echo").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("seats/lead.md"), root.join("seats/link.md")).unwrap();
    let files = load_package(root).unwrap();
    assert_eq!(files, load_package(&fixture()).unwrap());
}

#[test]
fn a_missing_directory_is_an_io_error() {
    let missing = fixture().join("absent");
    assert!(matches!(
        load_package(&missing),
        Err(Error::PackageIo { path, .. }) if path == missing
    ));
}

#[test]
fn a_non_utf8_document_is_refused() {
    let dir = copy();
    let bad = dir.path().join("seats/lead.md");
    std::fs::write(&bad, [0xff, 0xfe, 0x00]).unwrap();
    assert!(matches!(
        load_package(dir.path()),
        Err(Error::PackageEncoding { path }) if path == bad
    ));
}

#[test]
fn files_that_are_not_a_valid_package_are_refused() {
    let dir = copy();
    std::fs::remove_file(dir.path().join("roles/lead.md")).unwrap();
    assert!(matches!(load_package(dir.path()), Err(Error::Package(_))));
    let mut files = load_package(&fixture()).unwrap();
    files.insert("hive.json".into(), "{".into());
    assert!(matches!(editing_seats(&files), Err(Error::Package(_))));
}

#[test]
fn editing_seats_are_those_acting_in_a_candidate() {
    let files = load_package(&fixture()).unwrap();
    assert_eq!(editing_seats(&files).unwrap(), ["implementer-a"]);
}

#[test]
fn every_seat_carries_its_role_and_charter_body() {
    let mut files = load_package(&fixture()).unwrap();
    files.insert(
        "roles/lead.md".into(),
        "---\n{\"id\":\"lead\",\"charter\":\"roles/lead.md\"}\n---\nOwns the epoch.\n".into(),
    );
    let hive = Hive::open(&files).unwrap();
    let lead = &hive.seats[0];
    assert_eq!(lead.seat.id, "lead");
    assert_eq!(lead.role, "lead");
    assert_eq!(lead.charter, "Owns the epoch.\n");
    assert!(!lead.editing);
    let implementer = &hive.seats[1];
    assert_eq!(implementer.role, "implementer");
    assert!(implementer.charter.starts_with("Makes the assigned change"));
    assert!(implementer.editing);
    assert_eq!(hive.lowered.desks[0].id, "lab");
}

#[test]
fn keeps_only_package_file_kinds() {
    assert!(keeps(true, "hive.json"));
    assert!(keeps(true, "HIVE.JSON"));
    assert!(!keeps(false, "nested.json"));
    assert!(keeps(false, "lead.md"));
    assert!(!keeps(false, "README.md"));
    assert!(!keeps(true, "notes.md"));
    assert!(!keeps(false, "Makefile"));
}
