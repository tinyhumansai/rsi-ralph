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
    let (ok, out) = exec(
        &[
            "--home",
            home.path().to_str().unwrap(),
            "check",
            dir.to_str().unwrap(),
        ],
        true,
    );
    assert!(ok.is_ok());
    assert_eq!(
        out.trim(),
        "ok: graph-coloring (dir subject, 1 scorers, decide colors by seeds, hive lead lead)"
    );
    let (missing, _) = exec(
        &[
            "--home",
            home.path().to_str().unwrap(),
            "check",
            dir.to_str().unwrap(),
        ],
        false,
    );
    assert_eq!(exit_code(&missing), 2);
    assert!(matches!(missing, Err(Error::MissingKeys { .. })));
    let (bad, _) = exec(&["check", "/nonexistent/loop"], true);
    assert_eq!(exit_code(&bad), 1);
}

#[test]
fn check_names_parallel_seeds_only_when_raised() {
    let mut m = load(&example()).unwrap().manifest;
    assert!(!check_summary(&m).contains("at once"));
    m.score.parallel = 3;
    assert_eq!(
        check_summary(&m),
        "ok: graph-coloring (dir subject, 1 scorers, decide colors by seeds, 3 seeds at once, hive lead lead)"
    );
}

#[test]
fn score_scores_the_incumbent_with_the_loop_scorers() {
    let home = tempfile::tempdir().unwrap();
    let dir = example();
    let (ok, out) = exec(
        &[
            "--home",
            home.path().to_str().unwrap(),
            "score",
            dir.to_str().unwrap(),
            "--label",
            "base",
        ],
        true,
    );
    assert!(ok.is_ok(), "{ok:?} {out}");
    assert!(out.contains("colors\tseed 1\t"), "{out}");
    assert!(out.contains("colors = 17.0000"), "{out}");
}

#[test]
fn runs_and_show_read_the_store() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::new(home.path());
    let run = store
        .create_run("demo", Some("r1"), &BTreeMap::new())
        .unwrap();
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
            warnings: Vec::new(),
        },
    )
    .unwrap();
    let h = home.path().to_str().unwrap();
    let (ok, out) = exec(&["--home", h, "runs"], true);
    assert!(ok.is_ok());
    assert_eq!(out.trim(), "demo/r1\tfailed\tepochs 2\tscore 3.0000\t$0.50");
    let (ok, out) = exec(&["--home", h, "show", "demo/r1"], true);
    assert!(ok.is_ok());
    for line in [
        "status    failed",
        "stopped   error",
        "incumbent abc (3.0000)",
        "error     boom",
    ] {
        assert!(out.contains(line), "{line} in {out}");
    }
    let (missing, _) = exec(&["--home", h, "show", "demo/none"], true);
    assert_eq!(exit_code(&missing), 2);
    let (empty, out) = exec(&["--home", h, "runs", "--loop", "other"], true);
    assert!(empty.is_ok() && out.is_empty());
}

#[test]
fn show_names_a_provider_refusal_and_the_warnings() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::new(home.path());
    let run = store
        .create_run("demo", Some("r3"), &BTreeMap::new())
        .unwrap();
    write_json(
        &run.record_path(),
        &RunRecord {
            id: "r3".into(),
            r#loop: "demo".into(),
            status: RunStatus::Failed,
            pid: 1,
            started_at: 1,
            finished_at: Some(2),
            stop_reason: Some(tinyrsi_trace::StopReason::ProviderRefused),
            incumbent: "abc".into(),
            incumbent_score: Some(0.0),
            epochs: 2,
            spend_usd: 0.0,
            error: Some("provider refused 2 of 2 model calls (403: Key limit exceeded)".into()),
            warnings: vec!["epoch 1: provider refused 1 of 3 model calls (429: slow down)".into()],
        },
    )
    .unwrap();
    let h = home.path().to_str().unwrap();
    let (ok, out) = exec(&["--home", h, "show", "demo/r3"], true);
    assert!(ok.is_ok());
    for line in [
        "stopped   provider_refused",
        "error     provider refused 2 of 2 model calls (403: Key limit exceeded)",
        "warning   epoch 1: provider refused 1 of 3 model calls (429: slow down)",
    ] {
        assert!(out.contains(line), "{line} in {out}");
    }
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

/// A writer whose lines arrive on a channel, for a command that never returns.
struct Lines(std::sync::mpsc::Sender<String>);

impl Write for Lines {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = self.0.send(String::from_utf8_lossy(buf).into_owned());
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn serve_serves_the_store_until_stopped() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path().to_str().unwrap().to_owned();
    let (sender, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let cli = Cli::parse_from(["tinyrsi", "--home", &h, "serve", "--port", "0"]);
        let _ = execute(cli, &env(true), &mut Lines(sender));
    });
    let line = lines.recv().unwrap();
    let url = line
        .trim()
        .strip_prefix("serving http://")
        .unwrap()
        .to_owned();
    let mut stream = std::net::TcpStream::connect(url).unwrap();
    write!(stream, "GET /api/runs HTTP/1.1\r\n\r\n").unwrap();
    let mut reply = String::new();
    std::io::Read::read_to_string(&mut stream, &mut reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 200") && reply.ends_with("[]"));
}

#[test]
fn run_without_a_key_fails_before_starting_anything() {
    let home = tempfile::tempdir().unwrap();
    let dir = example();
    let (result, out) = exec(
        &[
            "--home",
            home.path().to_str().unwrap(),
            "run",
            dir.to_str().unwrap(),
            "--no-serve",
        ],
        false,
    );
    assert!(matches!(result, Err(Error::MissingKeys { .. })));
    assert_eq!(out, "");
    assert_eq!(Store::new(home.path()).list_runs(None).unwrap(), vec![]);
}

#[test]
fn score_accepts_an_explicit_checkout_and_show_skips_absent_fields() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path().to_str().unwrap();
    let dir = example();
    let checkout = dir.join("subject");
    let (ok, out) = exec(
        &[
            "--home",
            h,
            "score",
            dir.to_str().unwrap(),
            "--checkout",
            checkout.to_str().unwrap(),
        ],
        true,
    );
    assert!(ok.is_ok(), "{ok:?}");
    assert!(out.contains("colors = 17.0000"));
    let runs = Store::new(home.path()).list_runs(None).unwrap();
    assert!(runs.is_empty(), "a scoring folder is not a run");
    let store = Store::new(home.path());
    let run = store
        .create_run("demo", Some("r2"), &BTreeMap::new())
        .unwrap();
    write_json(
        &run.record_path(),
        &RunRecord {
            id: "r2".into(),
            r#loop: "demo".into(),
            status: RunStatus::Finished,
            pid: 1,
            started_at: 1,
            finished_at: None,
            stop_reason: None,
            incumbent: "abc".into(),
            incumbent_score: None,
            epochs: 0,
            spend_usd: 0.0,
            error: None,
            warnings: Vec::new(),
        },
    )
    .unwrap();
    let (ok, out) = exec(&["--home", h, "show", "demo/r2"], true);
    assert!(ok.is_ok());
    assert!(
        out.contains("incumbent abc (-)") && !out.contains("stopped") && !out.contains("error")
    );
}

#[test]
fn env_overrides_merge_into_the_manifest_and_reject_malformed_pairs() {
    let files = manifest::load_dir(&example()).unwrap();
    let merged = with_env(files.clone(), &["LIMIT=5".into(), "MODE=a=b".into()]).unwrap();
    let parsed = manifest::parse(&merged).unwrap();
    assert_eq!(
        parsed.manifest.env.get("LIMIT").map(String::as_str),
        Some("5")
    );
    assert_eq!(
        parsed.manifest.env.get("MODE").map(String::as_str),
        Some("a=b")
    );
    assert_eq!(with_env(files.clone(), &[]).unwrap(), files);
    for bad in ["NOEQUALS", "=value"] {
        let err = with_env(files.clone(), &[bad.into()]).unwrap_err();
        assert!(matches!(err, Error::Field { .. }), "{bad}: {err:?}");
    }
    let reserved = with_env(files, &["RSI_OUT=x".into()]).unwrap();
    assert!(matches!(
        manifest::parse(&reserved),
        Err(Error::Field { .. })
    ));
}

#[test]
fn score_passes_env_overrides_to_the_scorer() {
    let home = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("loop");
    crate::subject::copy_tree(&example(), &dir).unwrap();
    std::fs::write(
        dir.join("scorers/colors.py"),
        "import os\nprint(f\"METRIC colors={os.environ['COLORS']}\")\n",
    )
    .unwrap();
    let (ok, out) = exec(
        &[
            "--home",
            home.path().to_str().unwrap(),
            "score",
            dir.to_str().unwrap(),
            "--env",
            "COLORS=4",
        ],
        true,
    );
    assert!(ok.is_ok(), "{ok:?} {out}");
    assert!(out.contains("colors = 4.0000"), "{out}");
}
