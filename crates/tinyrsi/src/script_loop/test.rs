//! Unit tests for measurement, proposal validation, execution, scoring and the
//! supervisor, plus the shared fixtures the loop tests use.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::{score::*, supervisor::*, *};
use crate::{
    EvalPolicy, MetricDirection, ModelFuture, ModelPort, ModelReply, ModelRequest, ScoreDecision,
};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex},
};

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

pub(super) fn supervisor(port: Arc<Scripted>) -> Supervisor {
    Supervisor::new(port, SupervisorLimits::default()).unwrap()
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

fn proposal(paths: &[&str]) -> ScriptProposal {
    ScriptProposal {
        rationale: "r".into(),
        hypothesis: "h".into(),
        files: paths
            .iter()
            .map(|p| ProposedFile {
                path: (*p).into(),
                content: "x".into(),
            })
            .collect(),
    }
}

#[test]
fn rejects_proposal_paths_outside_the_editable_set() {
    let editable = vec![PathBuf::from("run.sh"), PathBuf::from("lib/model.py")];
    assert!(
        proposal(&["run.sh", "lib/model.py"])
            .validate(&editable)
            .is_ok()
    );
    for bad in [
        "/etc/passwd",
        "../run.sh",
        "lib/../run.sh",
        "./run.sh",
        "other.sh",
        "",
    ] {
        assert_eq!(
            proposal(&[bad]).validate(&editable).unwrap_err(),
            Error::ProposalPathOutsideEditable,
            "{bad}"
        );
    }
}

#[test]
fn rejects_empty_or_repeated_proposals_as_malformed() {
    let editable = vec![PathBuf::from("run.sh")];
    assert_eq!(
        proposal(&[]).validate(&editable).unwrap_err(),
        Error::SupervisorMalformed
    );
    assert_eq!(
        proposal(&["run.sh", "run.sh"])
            .validate(&editable)
            .unwrap_err(),
        Error::SupervisorMalformed
    );
    let mut blank = proposal(&["run.sh"]);
    blank.rationale = " ".into();
    assert_eq!(
        blank.validate(&editable).unwrap_err(),
        Error::SupervisorMalformed
    );
}

#[test]
fn content_ids_are_short_stable_and_content_sensitive() {
    let a: Sources = [(PathBuf::from("a"), "bc".to_owned())].into();
    let b: Sources = [(PathBuf::from("ab"), "c".to_owned())].into();
    assert_eq!(content_id(&a).len(), 12);
    assert_eq!(content_id(&a), content_id(&a.clone()));
    assert_ne!(content_id(&a), content_id(&b));
    let applied = proposal(&["a"]).apply(&a);
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
fn quality_preserves_metric_order_in_both_directions() {
    let min = MetricDirection::Minimize;
    let max = MetricDirection::Maximize;
    assert!(quality(0.1, min) > quality(0.2, min));
    assert!(quality(0.2, max) > quality(0.1, max));
    for m in [-1e9, -1.0, 0.0, 1.0, 1e9] {
        let q = quality(m, min);
        assert!(q > 0.0 && q < 1.0);
    }
    assert!(reaches(0.05, 0.05, min) && !reaches(0.06, 0.05, min));
    assert!(reaches(0.9, 0.8, max) && !reaches(0.7, 0.8, max));
    assert!(better(0.1, 0.2, min) && !better(0.2, 0.2, min));
}

#[test]
fn reports_the_interval_in_metric_units_with_positive_meaning_better() {
    let min = MetricDirection::Minimize;
    assert_eq!(interval(&[], &[], min, 2.0), (0.0, 0.0, 0.0));
    assert_eq!(
        interval(&[Some(0.5), Some(0.5)], &[Some(0.3), None], min, 2.0),
        (0.2, 0.2, 0.2)
    );
    let (estimate, lower, upper) =
        interval(&[Some(1.0), Some(1.0)], &[Some(0.5), Some(0.7)], min, 2.0);
    assert!((estimate - 0.4).abs() < 1e-12);
    assert!((lower - 0.2).abs() < 1e-12 && (upper - 0.6).abs() < 1e-12);
}

#[test]
fn scores_with_the_paired_evaluation_policy() {
    let policy = LoopPolicy::default().eval;
    let min = MetricDirection::Minimize;
    let better = score(&[1, 2], &[0.51, 0.52], &[0.31, 0.32], min, &policy).unwrap();
    assert_eq!(better.decision, ScoreDecision::Accept);
    assert!((better.estimate - 0.2).abs() < 1e-12);
    let worse = score(&[1, 2], &[0.51, 0.52], &[0.71, 0.72], min, &policy).unwrap();
    assert_eq!(worse.decision, ScoreDecision::Reject);
    assert!(worse.upper < 0.0);
    let noisy = score(&[1, 2], &[0.5, 0.5], &[0.3, 0.6], min, &policy).unwrap();
    assert_eq!(noisy.decision, ScoreDecision::Inconclusive);
    let invalid = EvalPolicy {
        standard_error_multiplier: 1.0,
        ..policy
    };
    assert!(score(&[1, 2], &[0.5, 0.5], &[0.3, 0.3], min, &invalid).is_err());
}

#[test]
fn supervisor_rejects_unusable_limits() {
    let port = Scripted::new(vec![]);
    for limits in [
        SupervisorLimits {
            call_tokens: 0,
            ..SupervisorLimits::default()
        },
        SupervisorLimits {
            output_tokens: 0,
            ..SupervisorLimits::default()
        },
        SupervisorLimits {
            output_tokens: 300_000,
            ..SupervisorLimits::default()
        },
        SupervisorLimits {
            timeout_ms: 0,
            ..SupervisorLimits::default()
        },
        SupervisorLimits {
            timeout_ms: 300_001,
            ..SupervisorLimits::default()
        },
    ] {
        assert_eq!(
            Supervisor::new(port.clone(), limits).unwrap_err(),
            Error::InvalidScriptSubject
        );
    }
    assert!(format!("{:?}", supervisor(port)).contains("limits"));
}

#[test]
fn supervisor_decodes_one_tool_call_and_flags_malformed_replies() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let port = Scripted::new(vec![
        reply(&loss("0.3")),
        "not json".into(),
        r#"{"rationale":"r","hypothesis":"h","files":[],"extra":1}"#.into(),
    ]);
    let sup = supervisor(port.clone());
    let turn = sup.propose(&s, "prompt".into(), 9).unwrap();
    assert_eq!(turn.tokens, 150);
    assert_eq!(turn.proposal.unwrap().files[0].content, loss("0.3"));
    for _ in 0..2 {
        let turn = sup.propose(&s, "prompt".into(), 9).unwrap();
        assert_eq!(turn.proposal.unwrap_err(), Error::SupervisorMalformed);
    }
    assert_eq!(
        sup.propose(&s, "prompt".into(), 9).unwrap_err(),
        Error::HostFailed
    );
    assert_eq!(port.seen.lock().unwrap()[0].1, 9);
    assert_eq!(
        schema(&s)["properties"]["files"]["items"]["properties"]["path"]["enum"][0],
        "run.sh"
    );
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

#[test]
fn the_scoring_crate_reproduces_script_loop_scores_exactly() {
    use tinyrsi_score::{Goal, MetricScorer, Scorer, decide};
    // A small deterministic generator, so the cases are varied but fixed.
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        f64::from(u32::try_from(state >> 40).unwrap()) / f64::from(1_u32 << 24) * 4.0 - 2.0
    };
    let goal = Goal::new("improve the metric");
    for case in 0..200_u64 {
        let direction = if case % 2 == 0 {
            crate::MetricDirection::Minimize
        } else {
            crate::MetricDirection::Maximize
        };
        let seeds: Vec<u64> = (0..2 + case % 4).collect();
        let incumbent: Vec<f64> = seeds.iter().map(|_| next()).collect();
        let shift = next();
        let candidate: Vec<f64> = incumbent.iter().map(|m| m + shift + next() * 0.1).collect();
        let policy = crate::EvalPolicy {
            margin: if case % 3 == 0 { 0.0 } else { 0.01 },
            ..crate::EvalPolicy::default()
        };
        let old = score(&seeds, &incumbent, &candidate, direction, &policy).unwrap();
        let scorer = MetricScorer::new(direction);
        let new = decide(
            &scorer.assess(&incumbent, &goal, &seeds).unwrap(),
            &scorer.assess(&candidate, &goal, &seeds).unwrap(),
            &seeds,
            &policy,
        )
        .unwrap();
        assert_eq!(new.decision, old.decision, "case {case}");
        assert_eq!(
            new.estimate.to_bits(),
            old.estimate.to_bits(),
            "case {case}"
        );
        assert_eq!(new.lower.to_bits(), old.lower.to_bits(), "case {case}");
        assert_eq!(new.upper.to_bits(), old.upper.to_bits(), "case {case}");
    }
}
