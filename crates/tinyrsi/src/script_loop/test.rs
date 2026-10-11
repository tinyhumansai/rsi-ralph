//! Unit tests for measurement, proposal validation, execution, scoring and the
//! supervisor, plus the shared fixtures the loop tests use.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::{prompt::*, score::*, *};
use crate::{MetricDirection, ModelFuture, ModelPort, ModelReply, ModelRequest};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex},
};
use tinyrsi_evolve::{ScriptSupervisor, SupervisorLimits};

/// A model port that replays canned tool-call arguments, then fails.
pub(super) struct Scripted {
    replies: Mutex<VecDeque<String>>,
    pub seen: Mutex<Vec<(String, u64)>>,
}

impl Scripted {
    pub(super) fn new(replies: Vec<String>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            seen: Mutex::new(vec![]),
        })
    }
}

impl ModelPort for Scripted {
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        self.seen
            .lock()
            .unwrap()
            .push((request.prompt().to_owned(), request.seed()));
        let next = self.replies.lock().unwrap().pop_front();
        Box::pin(async move {
            let content = next.ok_or(crate::ModelError::CallFailed)?.into_bytes();
            Ok(ModelReply {
                content,
                input_tokens: 100,
                output_tokens: 50,
            })
        })
    }
}

/// Tool arguments rewriting `run.sh` with `script`.
pub(super) fn reply(script: &str) -> String {
    serde_json::json!({
        "rationale": "lower the loss",
        "hypothesis": "loss drops",
        "files": [{"path": "run.sh", "content": script}]
    })
    .to_string()
}

/// A script printing `loss=<prefix><seed>`, e.g. `0.51` for prefix `0.5`.
pub(super) fn loss(prefix: &str) -> String {
    format!("echo \"METRIC loss={prefix}$TINYRSI_SEED\"\n")
}

/// A workspace holding `run.sh` and a subject that runs it on seeds 1 and 2.
pub(super) fn subject(dir: &Path, script: &str) -> ScriptSubject {
    let workspace = dir.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(workspace.join("run.sh"), script).unwrap();
    std::fs::write(workspace.join("data.txt"), "fixed").unwrap();
    ScriptSubject {
        name: "fixture".into(),
        brief: "Minimize the loss.".into(),
        workspace,
        editable: vec!["run.sh".into()],
        command: ScriptCommand {
            program: "/bin/sh".into(),
            args: vec!["run.sh".into()],
        },
        metric: MetricSpec {
            name: "loss".into(),
            direction: MetricDirection::Minimize,
            target: None,
        },
        seeds: vec![1, 2],
        timeout_ms: 5_000,
        output_cap: 10_000,
        env: vec![("EXTRA".into(), "on".into())],
    }
}

pub(super) fn supervisor(port: Arc<Scripted>) -> ScriptSupervisor {
    ScriptSupervisor::new(port, SupervisorLimits::default()).unwrap()
}

#[test]
fn parses_the_last_matching_metric_line() {
    let out = "noise\nMETRIC other=9\nMETRIC loss=0.5\n  METRIC loss = 0.25  \n";
    assert!((parse_metric(out, "loss").unwrap() - 0.25).abs() < f64::EPSILON);
    assert!((parse_metric(out, "other").unwrap() - 9.0).abs() < f64::EPSILON);
}

#[test]
fn reports_a_missing_metric() {
    assert_eq!(
        parse_metric("METRIC other=1\nloss=2\n", "loss").unwrap_err(),
        Error::MetricMissing
    );
}

#[test]
fn reports_non_finite_or_unparsable_metrics() {
    for value in ["nan", "inf", "-inf", "fast", ""] {
        assert_eq!(
            parse_metric(&format!("METRIC loss={value}\n"), "loss").unwrap_err(),
            Error::MetricInvalid
        );
    }
}

#[test]
fn content_ids_are_short_stable_and_content_sensitive() {
    let a: Sources = [(PathBuf::from("a"), "bc".to_owned())].into();
    let b: Sources = [(PathBuf::from("ab"), "c".to_owned())].into();
    assert_eq!(content_id(&a).len(), 12);
    assert_eq!(content_id(&a), content_id(&a.clone()));
    assert_ne!(content_id(&a), content_id(&b));
    let rewrite = tinyrsi_evolve::ScriptProposal {
        rationale: "r".into(),
        hypothesis: "h".into(),
        files: vec![tinyrsi_evolve::ProposedFile {
            path: "a".into(),
            content: "x".into(),
        }],
    };
    let applied = apply(&rewrite, &a);
    assert_eq!(applied[Path::new("a")], "x");
}

type Change = Box<dyn Fn(&mut ScriptSubject)>;

#[test]
fn validates_subjects_before_touching_files() {
    let dir = tempfile::tempdir().unwrap();
    let good = subject(dir.path(), &loss("0.5"));
    assert!(validate_subject(&good).is_ok());
    let cases: Vec<Change> = vec![
        Box::new(|s| s.name = " ".into()),
        Box::new(|s| s.metric.name = "a=b".into()),
        Box::new(|s| s.metric.target = Some(f64::NAN)),
        Box::new(|s| s.workspace = "/nonexistent/workspace".into()),
        Box::new(|s| s.editable.clear()),
        Box::new(|s| s.editable.push("run.sh".into())),
        Box::new(|s| s.editable = vec!["../run.sh".into()]),
        Box::new(|s| s.seeds = vec![1]),
        Box::new(|s| s.seeds = vec![1, 1]),
        Box::new(|s| s.timeout_ms = 0),
        Box::new(|s| s.output_cap = 0),
        Box::new(|s| s.command.program = PathBuf::new()),
        Box::new(|s| s.env.push(("TINYRSI_SEED".into(), "9".into()))),
    ];
    for change in cases {
        let mut bad = good.clone();
        change(&mut bad);
        assert_eq!(validate_subject(&bad), Err(Error::InvalidScriptSubject));
    }
    let mut missing = good.clone();
    missing.editable = vec!["absent.sh".into()];
    assert_eq!(read_sources(&missing), Err(Error::InvalidScriptSubject));
}

#[test]
fn materializes_the_workspace_with_overlaid_sources_and_skips_the_run_dir() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = s.workspace.join("runs");
    std::fs::create_dir_all(run.join("old")).unwrap();
    std::fs::create_dir_all(s.workspace.join("nested")).unwrap();
    std::fs::write(s.workspace.join("nested/keep.txt"), "k").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc", s.workspace.join("link")).unwrap();
    let dest = run.join("candidate");
    let sources: Sources = [(PathBuf::from("run.sh"), "new".to_owned())].into();
    materialize(&s.workspace, &run, &dest, &sources).unwrap();
    // A second materialization replaces the first.
    materialize(&s.workspace, &run, &dest, &sources).unwrap();
    assert_eq!(std::fs::read_to_string(dest.join("run.sh")).unwrap(), "new");
    assert_eq!(
        std::fs::read_to_string(dest.join("data.txt")).unwrap(),
        "fixed"
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("nested/keep.txt")).unwrap(),
        "k"
    );
    assert!(!dest.join("runs").exists());
    assert!(!dest.join("link").exists());
    let missing = materialize(Path::new("/nonexistent"), &run, &dest, &sources);
    assert_eq!(missing, Err(Error::HostFailed));
}

fn seed_with(script: &str, configure: impl Fn(&mut ScriptSubject)) -> SeedRun {
    let dir = tempfile::tempdir().unwrap();
    let mut s = subject(dir.path(), script);
    configure(&mut s);
    run_seed(&s, &s.workspace, &dir.path().join("tmp"), 7)
}

#[test]
fn runs_a_seed_with_only_the_documented_environment() {
    let run = seed_with(
        "echo \"seed=$TINYRSI_SEED extra=$EXTRA home=${HOME:-none} path=$PATH\" >&2\n\
         test -d \"$TMPDIR\" || exit 9\n\
         echo \"METRIC loss=0.$TINYRSI_SEED\"\n",
        |_| {},
    );
    assert_eq!(run.metric, Some(0.7));
    assert_eq!(run.error, None);
    assert!(
        run.stderr_tail
            .starts_with("seed=7 extra=on home=none path=/usr/bin:/bin")
    );
}

#[test]
fn reports_a_timed_out_seed() {
    let run = seed_with("sleep 5\n", |s| s.timeout_ms = 50);
    assert_eq!(run.error, Some(Error::ScriptTimeout.to_string()));
    assert_eq!(run.metric, None);
}

#[test]
fn reports_a_seed_that_exceeds_its_output_cap() {
    let run = seed_with("while true; do echo flood; done\n", |s| s.output_cap = 64);
    assert_eq!(run.error, Some(Error::ScriptOutputExceeded.to_string()));
    // Output that lands before the poll notices is still over the cap.
    let run = seed_with("printf '%0200d' 0; echo 'METRIC loss=1'\n", |s| {
        s.output_cap = 64;
        s.timeout_ms = 60_000;
    });
    assert_eq!(run.error, Some(Error::ScriptOutputExceeded.to_string()));
}

#[test]
fn reports_failed_missing_and_invalid_seeds() {
    let failed = seed_with("echo 'METRIC loss=1'; echo boom >&2; exit 2\n", |_| {});
    assert_eq!(failed.error, Some(Error::ScriptFailed.to_string()));
    assert_eq!(failed.stderr_tail, "boom\n");
    let missing = seed_with("echo nothing\n", |_| {});
    assert_eq!(missing.error, Some(Error::MetricMissing.to_string()));
    let invalid = seed_with("echo 'METRIC loss=nan'\n", |_| {});
    assert_eq!(invalid.error, Some(Error::MetricInvalid.to_string()));
    let spawn = seed_with("", |s| s.command.program = "/nonexistent/sh".into());
    assert_eq!(spawn.error, Some(Error::HostFailed.to_string()));
    let tmp = tempfile::tempdir().unwrap();
    let blocked = tmp.path().join("file");
    std::fs::write(&blocked, "").unwrap();
    let s = subject(tmp.path(), &loss("0.5"));
    let run = run_seed(&s, &s.workspace, &blocked, 1);
    assert_eq!(run.error, Some(Error::HostFailed.to_string()));
}

#[test]
fn reports_a_failed_candidates_interval_in_metric_units() {
    let min = MetricDirection::Minimize;
    assert_eq!(failed_interval(&[], &[], min, 2.0), (0.0, 0.0, 0.0));
    assert_eq!(
        failed_interval(&[Some(0.5), Some(0.5)], &[Some(0.3), None], min, 2.0),
        (0.2, 0.2, 0.2)
    );
    let (estimate, lower, upper) =
        failed_interval(&[Some(1.0), Some(1.0)], &[Some(0.5), Some(0.7)], min, 2.0);
    assert!((estimate - 0.4).abs() < 1e-12);
    assert!((lower - 0.2).abs() < 1e-12 && (upper - 0.6).abs() < 1e-12);
}

#[test]
fn prompt_shows_goal_history_rejections_and_sources() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = subject(dir.path(), &loss("0.5"));
    s.metric.target = Some(0.1);
    let failed = SeedRun {
        seed: 1,
        metric: None,
        error: Some("script run timed out".into()),
        duration_ms: 1,
        stderr_tail: "Traceback: boom".into(),
    };
    let rejected = LedgerCandidate {
        epoch: 1,
        proposal_id: Some("p1-0".into()),
        id: Some("abc".into()),
        parent: Some("base".into()),
        rationale: "try sleeping".into(),
        hypothesis: "slower is better".into(),
        paths: vec!["run.sh".into()],
        runs: vec![failed],
        mean: None,
        decision: CandidateDecision::Reject,
        estimate: None,
        lower: None,
        upper: None,
        promoted: false,
        note: None,
    };
    let ledger = ScriptLedger {
        run_id: "r".into(),
        subject: "fixture".into(),
        metric: s.metric.clone(),
        seeds: s.seeds.clone(),
        tokens: 0,
        stop: None,
        incumbents: vec![IncumbentEntry {
            epoch: 0,
            id: "base".into(),
            metric: 0.515,
        }],
        candidates: vec![rejected],
    };
    let sources = read_sources(&s).unwrap();
    let text = prompt(&s, &ledger, &sources, 3);
    for needle in [
        "Minimize the loss.",
        "target: 0.1",
        "epoch 0: base = 0.515",
        "try sleeping",
        "script run timed out",
        "Traceback: boom",
        "## run.sh",
        "METRIC loss=0.5",
    ] {
        assert!(text.contains(needle), "{needle}");
    }
    let empty = ScriptLedger {
        candidates: vec![],
        ..ledger
    };
    assert!(prompt(&s, &empty, &sources, 3).contains("None yet."));
}
