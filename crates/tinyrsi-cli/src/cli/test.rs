//! The subcommands that need no model: check, score, runs, show, exit codes.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::store::{RunRecord, write_json};
use std::collections::BTreeMap;

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../loops/graph-coloring")
}

fn env(with_key: bool) -> impl Fn(&str) -> Option<String> {
    move |name: &str| match name {
        "OPENROUTER_API_KEY" if with_key => Some("sk-test".to_owned()),
        "PATH" | "HOME" => std::env::var(name).ok(),
        _ => None,
    }
}

fn exec(args: &[&str], with_key: bool) -> (Result<()>, String) {
    let cli = Cli::parse_from(std::iter::once("tinyrsi").chain(args.iter().copied()));
    let mut out = Vec::new();
    let result = execute(cli, &env(with_key), &mut out);
    (result, String::from_utf8(out).unwrap())
}

#[test]
fn check_reports_a_valid_loop_and_missing_keys() {
    let home = tempfile::tempdir().unwrap();
    let dir = example();
    let (ok, out) = exec(&["--home", home.path().to_str().unwrap(), "check", dir.to_str().unwrap()], true);
    assert!(ok.is_ok());
    assert_eq!(
        out.trim(),
        "ok: graph-coloring (dir subject, 1 scorers, decide colors by Seeds, hive lead lead)"
    );
    let (missing, _) = exec(&["--home", home.path().to_str().unwrap(), "check", dir.to_str().unwrap()], false);
    assert_eq!(exit_code(&missing), 2);
    assert!(matches!(missing, Err(Error::MissingKeys { .. })));
    let (bad, _) = exec(&["check", "/nonexistent/loop"], true);
    assert_eq!(exit_code(&bad), 1);
}

#[test]
fn score_scores_the_incumbent_with_the_loop_scorers() {
    let home = tempfile::tempdir().unwrap();
    let dir = example();
    let (ok, out) = exec(&["--home", home.path().to_str().unwrap(), "score", dir.to_str().unwrap(), "--label", "base"], true);
    assert!(ok.is_ok(), "{ok:?} {out}");
    assert!(out.contains("colors\tseed 1\t"), "{out}");
    assert!(out.contains("colors = 17.0000"), "{out}");
}

#[test]
fn runs_and_show_read_the_store() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::new(home.path());
    let run = store.create_run("demo", Some("r1"), &BTreeMap::new()).unwrap();
    write_json(
        &run.record_path(),
        &RunRecord {
            id: "r1".into(),
            r#loop: "demo".into(),
            status: RunStatus::Failed,
            pid: 1,
            started_at: 1,
            finished_at: Some(2),
            stop_reason: Some(tinyrsi_trace::StopReason::Error),
            incumbent: "abc".into(),
            incumbent_score: Some(3.0),
            epochs: 2,
            spend_usd: 0.5,
            error: Some("boom".into()),
        },
    )
    .unwrap();
    let h = home.path().to_str().unwrap();
    let (ok, out) = exec(&["--home", h, "runs"], true);
    assert!(ok.is_ok());
    assert_eq!(out.trim(), "demo/r1\tfailed\tepochs 2\tscore 3.0000\t$0.50");
    let (ok, out) = exec(&["--home", h, "show", "demo/r1"], true);
    assert!(ok.is_ok());
    for line in ["status    failed", "stopped   error", "incumbent abc (3.0000)", "error     boom"] {
        assert!(out.contains(line), "{line} in {out}");
    }
    let (missing, _) = exec(&["--home", h, "show", "demo/none"], true);
    assert_eq!(exit_code(&missing), 2);
    let (empty, out) = exec(&["--home", h, "runs", "--loop", "other"], true);
    assert!(empty.is_ok() && out.is_empty());
}

#[test]
fn every_status_has_a_name() {
    for (status, name) in [
        (RunStatus::Running, "running"),
        (RunStatus::Finished, "finished"),
        (RunStatus::Failed, "failed"),
        (RunStatus::Interrupted, "interrupted"),
    ] {
        assert_eq!(super::status(status), name);
    }
    assert_eq!(exit_code(&Ok(())), 0);
}
