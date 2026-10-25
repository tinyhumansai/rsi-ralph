//! The subcommands that need no model: check, score, runs, show, exit codes.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "assert deterministic fixtures"
)]
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
    let cli = Cli::parse_from(std::iter::once("rsi-ralph").chain(args.iter().copied()));
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
            stop_reason: Some(rsi_ralph_trace::StopReason::Error),
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
            stop_reason: Some(rsi_ralph_trace::StopReason::ProviderRefused),
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
        let cli = Cli::parse_from(["rsi-ralph", "--home", &h, "serve", "--port", "0"]);
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

#[test]
fn score_passes_plain_keys_to_steps_under_their_own_name() {
    let home = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("loop");
    crate::subject::copy_tree(&example(), &dir).unwrap();
    let manifest = dir.join("rsi.json");
    let mut m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    m["keys"] = serde_json::json!({ "ladder": "LADDER_TEST_KEY" });
    std::fs::write(&manifest, m.to_string()).unwrap();
    std::fs::write(
        dir.join("scorers/colors.py"),
        "import os\nprint(f\"METRIC colors={len(os.environ['LADDER_TEST_KEY'])}\")\n",
    )
    .unwrap();
    let lookup = |name: &str| match name {
        "OPENROUTER_API_KEY" => Some("sk-test".to_owned()),
        "LADDER_TEST_KEY" => Some("abcd".to_owned()),
        "PATH" | "HOME" => std::env::var(name).ok(),
        _ => None,
    };
    let cli = Cli::parse_from([
        "rsi-ralph",
        "--home",
        home.path().to_str().unwrap(),
        "score",
        dir.to_str().unwrap(),
    ]);
    let mut out = Vec::new();
    let result = execute(cli, &lookup, &mut out);
    let out = String::from_utf8(out).unwrap();
    assert!(result.is_ok(), "{result:?} {out}");
    assert!(out.contains("colors = 4.0000"), "{out}");
}

#[test]
fn init_writes_a_loop_that_check_accepts() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("colors");
    let (ok, out) = exec(&["init", dir.to_str().unwrap()], true);
    assert!(ok.is_ok());
    assert!(out.starts_with("created colors in "), "{out}");
    assert!(out.contains("rsi-ralph run"), "{out}");
    let (checked, line) = exec(&["check", dir.to_str().unwrap()], true);
    assert!(checked.is_ok(), "{checked:?}");
    assert!(line.starts_with("ok: colors "), "{line}");
    let (again, _) = exec(&["init", dir.to_str().unwrap()], true);
    assert!(matches!(again, Err(Error::NotEmpty(_))));
    assert_eq!(exit_code(&again), 2);
    let (unknown, _) = exec(
        &[
            "init",
            root.path().join("x").to_str().unwrap(),
            "--template",
            "nope",
        ],
        true,
    );
    assert_eq!(exit_code(&unknown), 2);
}

#[test]
fn init_lists_the_bundled_templates() {
    let (ok, out) = exec(&["init", "--list"], true);
    assert!(ok.is_ok());
    assert!(out.starts_with("graph-coloring "), "{out}");
    assert!(Cli::try_parse_from(["rsi-ralph", "init"]).is_err());
}

#[test]
fn doctor_reports_the_store_it_would_use() {
    let home = tempfile::tempdir().unwrap();
    let (_, out) = exec(&["--home", home.path().to_str().unwrap(), "doctor"], true);
    assert!(out.starts_with("rsi-ralph "), "{out}");
    assert!(out.contains("ok      OPENROUTER_API_KEY is set"), "{out}");
    assert!(out.contains(&format!("store   {}", home.path().display())));
}

#[test]
fn completions_print_a_script_naming_every_command() {
    let (ok, out) = exec(&["completions", "bash"], true);
    assert!(ok.is_ok());
    for command in ["init", "doctor", "check", "run", "serve"] {
        assert!(out.contains(command), "{command} missing from completions");
    }
}

#[test]
fn a_doctor_failure_is_a_loop_error() {
    let missing = Err(Error::Doctor {
        missing: vec!["git".into()],
    });
    assert_eq!(exit_code(&missing), 1);
}

#[test]
fn steps_inherit_what_windows_programs_need_to_start() {
    // Python on Windows aborts at startup without `SYSTEMROOT`, so a step
    // environment holding only `PATH` and `HOME` cannot run a scorer there.
    let lookup = |name: &str| match name {
        "PATH" => Some("C:\\bin".to_owned()),
        "SYSTEMROOT" => Some("C:\\Windows".to_owned()),
        "USERPROFILE" => Some("C:\\Users\\me".to_owned()),
        "OPENROUTER_API_KEY" => Some("sk-secret".to_owned()),
        _ => None,
    };
    let env = run::inherited(&lookup);
    let names: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
    assert!(names.contains(&"SYSTEMROOT"), "{names:?}");
    assert!(names.contains(&"USERPROFILE"), "{names:?}");
    assert!(!names.contains(&"OPENROUTER_API_KEY"), "{names:?}");
}

#[test]
fn the_budget_sets_the_gateway_completion_cap() {
    use crate::manifest::Budget;
    use rsi_ralph_gateway::Limits;
    let stock = Limits::default();
    assert_eq!(run::gateway_limits(&Budget::default()), stock);
    let raised = run::gateway_limits(&Budget {
        max_tokens: Some(65_536),
        ..Budget::default()
    });
    assert_eq!(raised.max_tokens_cap, 65_536);
    assert_eq!(raised.default_max_tokens, stock.default_max_tokens);
    assert_eq!(raised.default_reasoning, stock.default_reasoning);
    let lowered = run::gateway_limits(&Budget {
        max_tokens: Some(1_024),
        ..Budget::default()
    });
    assert_eq!(lowered.max_tokens_cap, 1_024);
    assert_eq!(lowered.default_max_tokens, 1_024);
}

/// A writable copy of the example loop and a store, both in `root`.
fn upgradable(root: &Path) -> (PathBuf, String) {
    let dir = root.join("loop");
    crate::subject::copy_tree(&example(), &dir).unwrap();
    (dir, root.join("home").to_str().unwrap().to_owned())
}

#[test]
fn upgrades_add_keeps_each_contributors_rationale_and_show_renders_them() {
    let root = tempfile::tempdir().unwrap();
    let (dir, h) = upgradable(root.path());
    let d = dir.to_str().unwrap();
    let (ok, out) = exec(
        &[
            "--home",
            h.as_str(),
            "upgrades",
            "add",
            d,
            "--id",
            "u-1",
            "--title",
            "Keep every seat working",
            "--author",
            "assistant:claude-code@claude-opus-5-5=diagnosed",
            "--rationale",
            "The lead ends each seat's work at its first report.",
            "--author",
            "operator:steven=approved",
            "--rationale",
            "Idle seats waste the epoch.",
            "--saw",
            "the scout stopped after 4 minutes",
            "--metric",
            "seats_busy_mean=7.2@graph-coloring/r9",
            "--claim",
            "c-17",
            "--diagnosis",
            "The lead prompt ends work at the first report.",
            "--change",
            "The lead answers every report with the next assignment.",
            "--file",
            "brief.md",
            "--file",
            "hive/#max_turns",
            "--expect",
            "seats_busy_mean>=10",
            "--epoch",
            "1",
        ],
        true,
    );
    assert!(ok.is_ok(), "{ok:?}");
    assert!(out.starts_with("recorded u-1 in "), "{out}");
    let log = crate::upgrades::read(&dir.join(crate::upgrades::LOG)).unwrap();
    let [crate::upgrades::Entry::Upgrade(u)] = log.as_slice() else {
        panic!("one upgrade expected: {log:?}");
    };
    assert_eq!(u.contributors.len(), 2);
    assert_eq!(
        u.contributors[1].rationale.as_deref(),
        Some("Idle seats waste the epoch.")
    );
    assert_eq!(u.evidence.len(), 3);
    assert_eq!(u.applies_from.epoch, Some(1));
    // No run yet: the brief has an after hash only; a folder has none.
    let brief = std::fs::read_to_string(dir.join("brief.md")).unwrap();
    assert_eq!(
        u.change.files[0].after.as_deref(),
        Some(crate::upgrades::digest(&brief).as_str())
    );
    assert_eq!(u.change.files[0].before, None);
    assert_eq!(u.change.files[1].after, None);
    let (ok, md) = exec(&["--home", h.as_str(), "upgrades", "show", d], true);
    assert!(ok.is_ok(), "{ok:?}");
    for line in [
        "# Upgrades: graph-coloring",
        "## Keep every seat working",
        "`u-1` · from the next run, epoch 1 · by assistant claude-code (claude-opus-5-5), diagnosed; operator steven, approved",
        "- assistant claude-code (claude-opus-5-5): The lead ends each seat's work at its first report.",
        "- operator steven: Idle seats waste the epoch.",
        "- `seats_busy_mean` >= 10",
        "- claim c-17",
    ] {
        assert!(md.contains(line), "{line} in {md}");
    }
    let (ok, json) = exec(
        &["--home", h.as_str(), "upgrades", "show", d, "--json"],
        true,
    );
    assert!(ok.is_ok());
    assert_eq!(json.lines().count(), 1);
    assert!(json.starts_with("{\"kind\":\"upgrade\""), "{json}");
}

#[test]
fn upgrades_add_rejects_bad_authors_and_unpaired_rationales() {
    let root = tempfile::tempdir().unwrap();
    let (dir, h) = upgradable(root.path());
    let d = dir.to_str().unwrap();
    let base = [
        "--home",
        h.as_str(),
        "upgrades",
        "add",
        d,
        "--title",
        "t",
        "--change",
        "c",
    ];
    let with = |extra: &[&str]| {
        let mut args = base.to_vec();
        args.extend_from_slice(extra);
        exec(&args, true).0
    };
    let bad = with(&["--author", "robot:r2"]);
    assert!(matches!(&bad, Err(Error::Field { field, .. }) if field == "--author robot:r2"));
    assert_eq!(exit_code(&bad), 2);
    let unpaired = with(&[
        "--author",
        "operator:o",
        "--rationale",
        "a",
        "--rationale",
        "b",
    ]);
    assert!(matches!(&unpaired, Err(Error::Field { field, .. }) if field == "--rationale"));
    assert!(with(&["--author", "operator:o", "--expect", "nonsense"]).is_err());
    assert!(with(&["--author", "operator:o", "--file", "../outside"]).is_err());
    assert!(!dir.join(crate::upgrades::LOG).exists());
    let (ok, _) = exec(
        &[
            "--home",
            h.as_str(),
            "upgrades",
            "add",
            d,
            "--id",
            "u-1",
            "--title",
            "t",
            "--author",
            "operator:o",
            "--change",
            "c",
        ],
        true,
    );
    assert!(ok.is_ok(), "{ok:?}");
    let (dup, _) = exec(
        &[
            "--home",
            h.as_str(),
            "upgrades",
            "add",
            d,
            "--id",
            "u-1",
            "--title",
            "t",
            "--author",
            "operator:o",
            "--change",
            "c",
        ],
        true,
    );
    assert!(matches!(&dup, Err(Error::UpgradeExists(id)) if id == "u-1"));
    assert_eq!(exit_code(&dup), 2);
}

#[test]
fn upgrades_add_reads_a_whole_record_from_json_and_hashes_against_the_last_run() {
    let root = tempfile::tempdir().unwrap();
    let (dir, h) = upgradable(root.path());
    let d = dir.to_str().unwrap();
    // A previous run froze the brief as it was.
    let store = Store::new(&h);
    let files = BTreeMap::from([
        ("rsi.json".to_owned(), "{}".to_owned()),
        ("brief.md".to_owned(), "old brief".to_owned()),
    ]);
    let run = store
        .create_run("graph-coloring", Some("r1"), &files)
        .unwrap();
    write_json(
        &run.record_path(),
        &RunRecord {
            id: "r1".into(),
            r#loop: "graph-coloring".into(),
            status: RunStatus::Finished,
            pid: 1,
            started_at: 1,
            finished_at: Some(2),
            stop_reason: None,
            incumbent: String::new(),
            incumbent_score: None,
            epochs: 1,
            spend_usd: 0.0,
            error: None,
            warnings: Vec::new(),
        },
    )
    .unwrap();
    let record = root.path().join("record.json");
    std::fs::write(
        &record,
        serde_json::json!({
            "title": "Seat proposes a shorter brief",
            "contributors": [
                { "author": { "kind": "hive", "seat": "lead" }, "role": "proposed",
                  "rationale": "The brief repeats the rules." },
                { "author": { "kind": "operator", "name": "steven" }, "role": "applied" }
            ],
            "change": { "summary": "Shortened the brief.", "files": [ { "path": "brief.md" } ] }
        })
        .to_string(),
    )
    .unwrap();
    let (ok, out) = exec(
        &[
            "--home",
            h.as_str(),
            "upgrades",
            "add",
            d,
            "--from",
            record.to_str().unwrap(),
        ],
        true,
    );
    assert!(ok.is_ok(), "{ok:?}");
    assert!(out.contains("recorded u-"), "{out}");
    let log = crate::upgrades::read(&dir.join(crate::upgrades::LOG)).unwrap();
    let [crate::upgrades::Entry::Upgrade(u)] = log.as_slice() else {
        panic!("one upgrade expected: {log:?}");
    };
    assert_eq!(
        u.contributors[0].author,
        crate::upgrades::Author::Hive {
            seat: "lead".into()
        }
    );
    assert_eq!(
        u.change.files[0].before.as_deref(),
        Some(crate::upgrades::digest("old brief").as_str())
    );
    assert!(u.change.files[0].after.is_some());
    // The loop's name alone reads the log from the latest run's frozen copy,
    // which predates the record.
    let (ok, md) = exec(
        &["--home", h.as_str(), "upgrades", "show", "graph-coloring"],
        true,
    );
    assert!(ok.is_ok(), "{ok:?}");
    assert!(md.contains("No upgrades recorded."), "{md}");
    let (none, _) = exec(
        &["--home", h.as_str(), "upgrades", "show", "no-such-loop"],
        true,
    );
    assert!(matches!(none, Err(Error::NoRun(_))));
    let (bad, _) = exec(
        &["--home", h.as_str(), "upgrades", "show", "not/a/loop"],
        true,
    );
    assert!(matches!(bad, Err(Error::MissingFile { .. })));
}

#[test]
fn upgrades_outcome_judges_the_predictions_against_the_measured_values() {
    let root = tempfile::tempdir().unwrap();
    let (dir, h) = upgradable(root.path());
    let d = dir.to_str().unwrap();
    let (ok, _) = exec(
        &[
            "--home",
            h.as_str(),
            "upgrades",
            "add",
            d,
            "--id",
            "u-1",
            "--title",
            "t",
            "--author",
            "operator:o",
            "--change",
            "c",
            "--expect",
            "busy>=10",
            "--expect",
            "p90_s<30",
        ],
        true,
    );
    assert!(ok.is_ok(), "{ok:?}");
    let outcome = |extra: &[&str]| {
        let mut args = vec!["--home", h.as_str(), "upgrades", "outcome", d];
        args.extend_from_slice(extra);
        exec(&args, true)
    };
    let (ok, out) = outcome(&[
        "u-1",
        "--run",
        "r2",
        "--metric",
        "busy=11",
        "--metric",
        "p90_s=45",
        "--author",
        "assistant:claude-code@claude-opus-5-5",
        "--rationale",
        "Busy, but slower.",
    ]);
    assert!(ok.is_ok(), "{ok:?}");
    assert_eq!(out.trim(), "recorded outcome of u-1 in run r2: mixed");
    let (ok, out) = outcome(&[
        "u-1",
        "--run",
        "r3",
        "--verdict",
        "refuted",
        "--saw",
        "stopped early",
    ]);
    assert!(ok.is_ok(), "{ok:?}");
    assert!(out.ends_with("refuted\n"), "{out}");
    let (unmeasured, _) = outcome(&["u-1", "--run", "r4", "--metric", "other=1"]);
    assert!(matches!(&unmeasured, Err(Error::Field { field, .. }) if field == "--verdict"));
    let (unknown, _) = outcome(&["u-9", "--run", "r4", "--verdict", "confirmed"]);
    assert!(matches!(
        &unknown,
        Err(Error::Reference {
            kind: "upgrade",
            ..
        })
    ));
    assert_eq!(exit_code(&unknown), 2);
    let (_, md) = exec(&["--home", h.as_str(), "upgrades", "show", d], true);
    for line in [
        "**Outcome in run r2: mixed** (by assistant claude-code (claude-opus-5-5), authored)",
        "- `busy` = 11: expected >= 10, held",
        "- `p90_s` = 45: expected < 30, did not hold",
        "- assistant claude-code (claude-opus-5-5): Busy, but slower.",
        "**Outcome in run r3: refuted**",
        "- stopped early",
    ] {
        assert!(md.contains(line), "{line} in {md}");
    }
}
