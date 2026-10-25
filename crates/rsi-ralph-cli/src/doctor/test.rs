//! Doctor checks against a fake `PATH`: found, missing, keys never printed.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;

/// A folder holding empty files named for `tools`, as a `PATH` entry.
fn bin(tools: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for tool in tools {
        let name = if cfg!(windows) {
            format!("{tool}.exe")
        } else {
            (*tool).to_owned()
        };
        std::fs::write(dir.path().join(name), "").unwrap();
    }
    dir
}

fn doctor(path: &Path, key: Option<&str>, home: &Path) -> (Result<()>, Vec<String>) {
    let path = path.to_str().unwrap().to_owned();
    let key = key.map(str::to_owned);
    let lookup = move |name: &str| match name {
        "PATH" => Some(path.clone()),
        MODEL_KEY => key.clone(),
        _ => None,
    };
    let mut lines = Vec::new();
    let result = run(home, &lookup, &mut |line| {
        lines.push(line.to_owned());
        Ok(())
    });
    (result, lines)
}

#[test]
fn passes_when_every_required_tool_is_on_path() {
    let tools = bin(&["git", "diff", "python3"]);
    let home = tempfile::tempdir().unwrap();
    let (result, lines) = doctor(tools.path(), Some("sk-secret"), home.path());
    assert!(result.is_ok());
    assert!(lines[0].starts_with("rsi-ralph "));
    assert!(lines.iter().any(|l| l.starts_with("ok      git")));
    assert!(
        lines
            .iter()
            .any(|l| l == "ok      OPENROUTER_API_KEY is set")
    );
    assert!(lines.iter().any(|l| l.ends_with("(exists)")));
    assert!(!lines.iter().any(|l| l.contains("sk-secret")));
}

#[test]
fn fails_naming_each_missing_required_tool() {
    let tools = bin(&["python3"]);
    let home = tempfile::tempdir().unwrap();
    let (result, lines) = doctor(tools.path(), None, &home.path().join("new"));
    assert!(
        matches!(&result, Err(Error::Doctor { missing }) if missing == &["git", "diff"]),
        "{result:?}"
    );
    assert!(lines.iter().any(|l| l.starts_with("missing git")));
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("absent  OPENROUTER_API_KEY"))
    );
    assert!(lines.iter().any(|l| l.ends_with("(created on first run)")));
}

#[test]
fn an_absent_optional_tool_does_not_fail() {
    let tools = bin(&["git", "diff"]);
    let home = tempfile::tempdir().unwrap();
    let (result, lines) = doctor(tools.path(), Some(""), home.path());
    assert!(result.is_ok());
    assert!(lines.iter().any(|l| l.starts_with("absent  python3")));
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("absent  OPENROUTER_API_KEY"))
    );
}

#[test]
fn finds_nothing_without_a_path() {
    assert!(find("git", None).is_none());
}

#[test]
fn passes_on_the_error_from_its_writer() {
    let home = tempfile::tempdir().unwrap();
    let result = run(home.path(), &|_| None, &mut |_| {
        Err(Error::NoRun("closed".into()))
    });
    assert!(matches!(result, Err(Error::NoRun(_))));
}
