//! Writing bundled templates: names, refusals, and loops that parse.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;

#[test]
fn writes_a_loop_that_parses_and_takes_the_folder_name() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("my-colors");
    let name = write(template("graph-coloring").unwrap(), &dir).unwrap();
    assert_eq!(name, "my-colors");
    let parsed = manifest::parse(&manifest::load_dir(&dir).unwrap()).unwrap();
    assert_eq!(parsed.manifest.name, "my-colors");
    assert!(dir.join("subject/coloring.py").is_file());
}

#[test]
fn every_template_matches_its_loop_folder() {
    for t in TEMPLATES {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../loops")
            .join(t.name);
        let on_disk = manifest::load_dir(&source).unwrap();
        for (path, text) in t.files {
            if let Some(disk) = on_disk.get(*path) {
                assert_eq!(disk, text, "{}/{path}", t.name);
            }
        }
        let written = tempfile::tempdir().unwrap();
        write(t, &written.path().join(t.name)).unwrap();
        let copied = manifest::load_dir(&written.path().join(t.name)).unwrap();
        assert_eq!(
            copied.keys().collect::<Vec<_>>(),
            on_disk.keys().collect::<Vec<_>>(),
            "template {} is missing files of loops/{}",
            t.name,
            t.name
        );
    }
}

#[test]
fn accepts_an_existing_empty_folder() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("empty");
    std::fs::create_dir(&dir).unwrap();
    assert!(write(&TEMPLATES[0], &dir).is_ok());
}

#[test]
fn refuses_a_folder_that_holds_files() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("taken");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("keep.txt"), "mine").unwrap();
    let err = write(&TEMPLATES[0], &dir).unwrap_err();
    assert!(matches!(err, Error::NotEmpty(_)), "{err}");
    assert_eq!(
        std::fs::read_to_string(dir.join("keep.txt")).unwrap(),
        "mine"
    );
}

#[test]
fn rejects_a_folder_without_a_usable_name() {
    let root = tempfile::tempdir().unwrap();
    let err = write(&TEMPLATES[0], &root.path().join(".hidden")).unwrap_err();
    assert!(matches!(err, Error::Field { .. }), "{err}");
    assert!(matches!(
        write(&TEMPLATES[0], Path::new("/")),
        Err(Error::Field { .. })
    ));
}

#[test]
fn reports_an_unknown_template() {
    assert!(matches!(
        template("nope"),
        Err(Error::Reference {
            kind: "template",
            ..
        })
    ));
}

#[test]
fn reports_a_manifest_that_is_not_json() {
    assert!(matches!(renamed("{", "x"), Err(Error::Json { .. })));
}

#[test]
fn reports_a_file_it_cannot_write() {
    let root = tempfile::tempdir().unwrap();
    let blocker = root.path().join("file");
    std::fs::write(&blocker, "").unwrap();
    let err = write(&TEMPLATES[0], &blocker.join("loop")).unwrap_err();
    assert!(matches!(err, Error::Io { .. }), "{err}");
}
