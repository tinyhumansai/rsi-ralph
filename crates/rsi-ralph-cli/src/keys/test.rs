//! Key resolution from the environment and `.env`, and redaction.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::manifest;

fn example() -> Loop {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../loops/graph-coloring");
    let mut files = manifest::load_dir(&dir).unwrap();
    let mut m: serde_json::Value = serde_json::from_str(&files["rsi.json"]).unwrap();
    m["keys"] =
        serde_json::json!({ "web_search": { "provider": "gemini", "env": "GEMINI_API_KEY" } });
    files.insert("rsi.json".into(), m.to_string());
    manifest::parse(&files).unwrap()
}

#[test]
fn resolves_from_the_environment_then_dotenv() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".env"),
        "# comment\nexport GEMINI_API_KEY=\"from-file\"\nOPENROUTER_API_KEY=shadowed\n",
    )
    .unwrap();
    let env = |name: &str| (name == "OPENROUTER_API_KEY").then(|| "from-env".to_owned());
    let keys = Keys::resolve(&example(), dir.path(), &env).unwrap();
    assert_eq!(keys.get("OPENROUTER_API_KEY").unwrap().expose(), "from-env");
    assert_eq!(keys.get("GEMINI_API_KEY").unwrap().expose(), "from-file");
    assert_eq!(keys.values().count(), 2);
}

#[test]
fn names_every_missing_key_without_values() {
    let dir = tempfile::tempdir().unwrap();
    let err = Keys::resolve(&example(), dir.path(), &|_| None).unwrap_err();
    assert!(
        matches!(&err, Error::MissingKeys { names } if names == &["GEMINI_API_KEY", "OPENROUTER_API_KEY"])
    );
    assert_eq!(
        err.to_string(),
        "missing keys: GEMINI_API_KEY, OPENROUTER_API_KEY"
    );
}

#[test]
fn an_unreadable_dotenv_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".env")).unwrap();
    assert!(matches!(
        Keys::resolve(&example(), dir.path(), &|_| None),
        Err(Error::Io { .. })
    ));
}

#[test]
fn secrets_never_print() {
    let secret = Secret("sk-live".into());
    assert_eq!(format!("{secret:?}"), "Secret(***)");
    let keys = Keys {
        values: BTreeMap::from([("K".to_owned(), secret)]),
    };
    assert!(!format!("{keys:?}").contains("sk-live"));
}

#[test]
fn dotenv_parsing_strips_quotes_and_ignores_noise() {
    let parsed = parse_dotenv("A='1'\n\nnot a pair\n B = 2 \n#C=3\n");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed["A"], "1");
    assert_eq!(parsed["B"], "2");
}
