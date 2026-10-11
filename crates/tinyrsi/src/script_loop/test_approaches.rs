//! Unit tests for the hypothesis ledger: approach grouping, the prompt
//! sections built from it, slot directives and the new ledger fields.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::approaches::*;
use super::prompt::{Slot, prompt};
use super::test::{Scripted, loss, subject, supervisor};
use super::*;
use crate::{LoopEvent, ManualClock, MetricDirection, VecTrace};
use execute::read_sources;

/// A proposal row of `epoch` with `approach`, `mean` and `decision`.
fn row(
    epoch: u64,
    approach: Option<&str>,
    mean: Option<f64>,
    decision: CandidateDecision,
) -> LedgerCandidate {
    LedgerCandidate {
        epoch,
        proposal_id: Some(format!("p{epoch}-0")),
        id: Some(format!("c{epoch}")),
        parent: Some("base".into()),
        rationale: "r".into(),
        hypothesis: "h".into(),
        paths: vec!["run.sh".into()],
        runs: vec![],
        mean,
        decision,
        estimate: None,
        lower: None,
        upper: None,
        units: IntervalUnits::Metric,
        promoted: false,
        note: None,
        approach: approach.map(str::to_owned),
        differs_from_previous: None,
        temperature: None,
    }
}

fn seed(error: Option<&str>, diagnostics: &[&str]) -> SeedRun {
    SeedRun {
        seed: 1,
        metric: error.is_none().then_some(1.0),
        error: error.map(str::to_owned),
        duration_ms: 1,
        stderr_tail: String::new(),
        diagnostics: diagnostics.iter().map(|d| (*d).to_owned()).collect(),
    }
}

fn ledger_of(candidates: Vec<LedgerCandidate>) -> ScriptLedger {
    ScriptLedger {
        run_id: "r".into(),
        subject: "fixture".into(),
        metric: MetricSpec {
            name: "loss".into(),
            direction: MetricDirection::Minimize,
            target: None,
        },
        seeds: vec![1, 2],
        tokens: 0,
        stop: None,
        incumbents: vec![IncumbentEntry {
            epoch: 0,
            id: "base".into(),
            metric: 0.5,
        }],
        candidates,
    }
}

#[test]
fn normalizes_case_whitespace_and_punctuation() {
    for label in ["Tabu-Search", "tabu search", " TABU  search! ", "tabu_search"] {
        assert_eq!(normalize(label), "tabu search", "{label}");
    }
    assert_eq!(normalize("DSATUR (v2)"), "dsatur v2");
    assert_eq!(normalize("Ünïcode Ök"), "ünïcode ök");
    assert_eq!(normalize(" -- "), "");
}

#[test]
fn groups_attempts_by_normalized_label_newest_first() {
    let mut baseline = row(0, Some("baseline"), Some(0.5), CandidateDecision::Baseline);
    baseline.proposal_id = None;
    let mut punctuation = row(3, Some("!!"), Some(0.1), CandidateDecision::Reject);
    punctuation.proposal_id = Some("p3-1".into());
    let ledger = ledger_of(vec![
        baseline,
        row(1, Some("Tabu search"), Some(0.4), CandidateDecision::Reject),
        row(1, Some("DSATUR"), Some(0.45), CandidateDecision::Accept),
        row(2, None, Some(0.2), CandidateDecision::Reject),
        row(2, Some("tabu-search"), Some(0.3), CandidateDecision::Reject),
        punctuation,
        row(3, Some("TABU SEARCH"), None, CandidateDecision::Duplicate),
    ]);
    let tried = summarize(&ledger, MetricDirection::Minimize, 12);
    assert_eq!(tried.len(), 2);
    let tabu = &tried[0];
    assert_eq!(tabu.label, "TABU SEARCH");
    assert_eq!(tabu.attempts, 3);
    assert_eq!(tabu.best, Some(0.3));
    assert_eq!(
        (tabu.decision, tabu.epoch),
        (CandidateDecision::Duplicate, 3)
    );
    assert_eq!(tried[1].label, "DSATUR");
    assert_eq!(tried[1].attempts, 1);
    // To maximize, the best of the same attempts is the largest.
    let max = summarize(&ledger, MetricDirection::Maximize, 12);
    assert_eq!(max[0].best, Some(0.4));
    // The bound keeps the most recently attempted approaches.
    let one = summarize(&ledger, MetricDirection::Minimize, 1);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].label, "TABU SEARCH");
}

#[test]
fn a_group_keeps_its_best_mean_when_a_later_attempt_failed() {
    let ledger = ledger_of(vec![
        row(1, Some("cubic"), None, CandidateDecision::Reject),
        row(2, Some("cubic"), Some(0.2), CandidateDecision::Reject),
        row(3, Some("cubic"), None, CandidateDecision::Reject),
    ]);
    let tried = summarize(&ledger, MetricDirection::Minimize, 12);
    assert_eq!((tried[0].attempts, tried[0].best), (3, Some(0.2)));
}

#[test]
fn the_verdict_prefers_an_error_then_a_note_then_the_last_diagnostic() {
    let mut failed = row(1, Some("a"), None, CandidateDecision::Reject);
    failed.runs = vec![
        seed(None, &["fine"]),
        seed(Some("script run timed out"), &["stuck"]),
    ];
    failed.note = Some("ignored".into());
    let mut noted = row(1, Some("b"), None, CandidateDecision::Duplicate);
    noted.runs = vec![seed(None, &["ignored"])];
    noted.note = Some("content already evaluated in this run".into());
    let mut diagnosed = row(1, Some("c"), Some(0.4), CandidateDecision::Reject);
    diagnosed.runs = vec![
        seed(None, &[]),
        seed(None, &["start", "Greedy recoloring: 0 moves"]),
    ];
    let silent = row(1, Some("d"), Some(0.4), CandidateDecision::Reject);
    let mut long = row(1, Some("e"), Some(0.4), CandidateDecision::Reject);
    long.runs = vec![seed(None, &[&"x ".repeat(VERDICT_CHARS)])];
    let tried = summarize(
        &ledger_of(vec![failed, noted, diagnosed, silent, long]),
        MetricDirection::Minimize,
        12,
    );
    let verdicts: Vec<&str> = tried.iter().rev().map(|a| a.verdict.as_str()).collect();
    assert_eq!(
        verdicts[..4],
        [
            "error: script run timed out",
            "content already evaluated in this run",
            "Greedy recoloring: 0 moves",
            "no diagnostics",
        ]
    );
    assert_eq!(verdicts[4].chars().count(), VERDICT_CHARS);
    assert!(verdicts[4].ends_with('…'));
}

#[test]
fn one_line_collapses_whitespace_and_cuts_long_text() {
    assert_eq!(one_line(" a \n\t b ", 10), "a b");
    assert_eq!(one_line("abcdef", 4), "abc…");
    assert_eq!(one_line("abcd", 4), "abcd");
}

#[test]
fn decision_names_match_the_ledger_spelling() {
    for decision in [
        CandidateDecision::Baseline,
        CandidateDecision::Accept,
        CandidateDecision::Reject,
        CandidateDecision::Inconclusive,
        CandidateDecision::Invalid,
        CandidateDecision::Duplicate,
        CandidateDecision::Error,
    ] {
        assert_eq!(
            serde_json::to_value(decision).unwrap(),
            decision_name(decision)
        );
    }
}

#[test]
fn renders_the_tried_section_with_its_resubmission_rule() {
    let mut retried = row(2, Some("Kempe chains"), Some(0.4), CandidateDecision::Reject);
    retried.differs_from_previous = Some("swap  two\ncolours, not one".into());
    retried.runs = vec![seed(None, &["After Kempe: 0 moves"])];
    let ledger = ledger_of(vec![
        row(1, Some("Kempe chains"), Some(0.45), CandidateDecision::Reject),
        retried,
        row(2, Some("DSATUR"), None, CandidateDecision::Reject),
    ]);
    let text = render_tried(&ledger, MetricDirection::Minimize, 12);
    assert_eq!(
        text,
        "# Approaches already tried (most recent first)\n\
         - \"DSATUR\": 1 attempt, best mean none, last decision reject (epoch 2); verdict: no diagnostics\n\
         - \"Kempe chains\": 2 attempts, best mean 0.4, last decision reject (epoch 2); verdict: After Kempe: 0 moves\n\
         \x20 last said it differed by: swap two colours, not one\n\
         Do not resubmit an approach whose last decision was not accept unless your proposal's \
         `differs_from_previous` states concretely what is different this time.\n\n"
    );
    assert_eq!(render_tried(&ledger, MetricDirection::Minimize, 0), "");
    let unlabelled = render_tried(
        &ledger_of(vec![]),
        MetricDirection::Minimize,
        12,
    );
    assert!(unlabelled.contains("None labelled yet.\nDo not resubmit"));
}

#[test]
fn renders_earlier_proposals_of_the_current_epoch_only() {
    let mut second = row(2, None, None, CandidateDecision::Invalid);
    second.proposal_id = Some("p2-1".into());
    let ledger = ledger_of(vec![
        row(1, Some("old"), Some(0.4), CandidateDecision::Reject),
        row(2, Some("tabu search"), Some(0.3), CandidateDecision::Accept),
        second,
    ]);
    assert_eq!(
        render_this_epoch(&ledger, 2),
        "# Earlier proposals this epoch\n\
         - p2-0: approach \"tabu search\", decision accept, mean 0.3\n\
         - p2-1: approach unlabelled, decision invalid, mean none\n\n"
    );
    assert_eq!(render_this_epoch(&ledger, 3), "");
}

#[test]
fn the_prompt_orders_tried_approaches_before_rejections_and_ends_with_the_directive() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let sources = read_sources(&s).unwrap();
    let ledger = ledger_of(vec![
        row(1, Some("sleep sort"), Some(0.6), CandidateDecision::Reject),
        row(2, Some("cache"), Some(0.4), CandidateDecision::Reject),
    ]);
    let policy = LoopPolicy {
        proposals_per_epoch: 3,
        ..LoopPolicy::default()
    };
    let text = prompt(&s, &ledger, &sources, &policy, Slot { epoch: 2, index: 1 });
    let at = |needle: &str| text.find(needle).unwrap();
    assert!(at("# Incumbent diagnostics") < at("# Approaches already tried"));
    assert!(at("\"sleep sort\": 1 attempt") < at("# Earlier proposals this epoch"));
    assert!(at("- p2-0: approach \"cache\"") < at("# Recently rejected proposals"));
    assert!(at("# Incumbent editable sources") < at("# This proposal (slot 2 of 3)"));
    assert!(text.trim_end().ends_with(&policy.slots[1].instruction));
    // A fourth slot cycles back to the first directive.
    let fourth = prompt(&s, &ledger, &sources, &policy, Slot { epoch: 2, index: 3 });
    assert!(fourth.contains("# This proposal (slot 4 of 3)"));
    assert!(fourth.trim_end().ends_with(&policy.slots[0].instruction));

    let bare = LoopPolicy {
        slots: vec![],
        approach_context: 0,
        ..LoopPolicy::default()
    };
    let text = prompt(&s, &ledger, &sources, &bare, Slot { epoch: 3, index: 0 });
    for absent in [
        "# Approaches already tried",
        "Do not resubmit",
        "# Earlier proposals this epoch",
        "# This proposal",
    ] {
        assert!(!text.contains(absent), "{absent}");
    }
    assert!(text.contains("\n\n# Recently rejected proposals"));
}

#[test]
fn default_slots_ask_three_different_questions_at_default_sampling() {
    let policy = LoopPolicy::default();
    assert_eq!(policy.approach_context, 12);
    assert_eq!(policy.slots, SlotDirective::defaults());
    assert_eq!(policy.slots.len(), 3);
    assert!(policy.slots.iter().all(|s| s.temperature.is_none()));
    assert!(policy.slots[0].instruction.starts_with("Best next step"));
    assert!(policy.slots[1].instruction.starts_with("Explore"));
    assert!(policy.slots[2].instruction.starts_with("Repair"));
    assert_eq!(policy.slot(4), policy.slots.get(1));
    let hot = SlotDirective::new("explore").with_temperature(0.9);
    assert_eq!(
        (hot.instruction.as_str(), hot.temperature),
        ("explore", Some(0.9))
    );
    let none = LoopPolicy {
        slots: vec![],
        ..policy
    };
    assert_eq!(none.slot(0), None);
}

#[test]
fn pins_ledger_rows_with_and_without_approach_fields() {
    let legacy = row(1, None, Some(0.4), CandidateDecision::Reject);
    let text = serde_json::to_string(&legacy).unwrap();
    assert!(!text.contains("approach") && !text.contains("temperature"));
    assert_eq!(
        serde_json::from_str::<LedgerCandidate>(&text).unwrap(),
        legacy
    );
    let mut full = row(1, Some("tabu search"), Some(0.4), CandidateDecision::Reject);
    full.differs_from_previous = Some("tenure 7".into());
    full.temperature = Some(0.7);
    let value = serde_json::to_value(&full).unwrap();
    assert_eq!(value["approach"], "tabu search");
    assert_eq!(value["differs_from_previous"], "tenure 7");
    assert_eq!(value["temperature"], 0.7);
    assert_eq!(
        serde_json::from_value::<LedgerCandidate>(value).unwrap(),
        full
    );
}

/// Tool arguments rewriting `run.sh` with `script`, labelled `approach`.
fn labelled(approach: &str, differs: Option<&str>, script: &str) -> String {
    serde_json::json!({
        "approach": approach,
        "differs_from_previous": differs,
        "rationale": "lower the loss",
        "hypothesis": "loss drops",
        "files": [{"path": "run.sh", "content": script}]
    })
    .to_string()
}

#[test]
fn later_slots_see_earlier_slots_and_sample_at_their_own_temperature() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = RunLocation {
        id: "run-1".into(),
        dir: dir.path().join("run"),
    };
    let port = Scripted::new(vec![
        labelled("Cheaper loss", None, &loss("0.3")),
        labelled("cheaper-loss!", Some("  a smaller prefix "), &loss("0.4")),
        "{not json".into(),
    ]);
    let policy = LoopPolicy {
        max_epochs: 1,
        proposals_per_epoch: 3,
        slots: vec![
            SlotDirective::new("first"),
            SlotDirective::new("second").with_temperature(0.8),
        ],
        ..LoopPolicy::default()
    };
    let mut trace = VecTrace::new(ManualClock::new());
    run_script_loop(&s, &supervisor(port.clone()), &policy, &run, &mut trace).unwrap();

    let seen = port.seen.lock().unwrap();
    assert!(!seen[0].0.contains("# Earlier proposals this epoch"));
    assert!(seen[0].0.contains("None labelled yet."));
    assert!(seen[0].0.trim_end().ends_with("first"));
    assert!(
        seen[1]
            .0
            .contains("- p1-0: approach \"Cheaper loss\", decision accept, mean 0.315")
    );
    assert!(seen[1].0.contains("\"Cheaper loss\": 1 attempt"));
    assert!(seen[1].0.trim_end().ends_with("second"));
    assert!(seen[2].0.contains("\"cheaper-loss!\": 2 attempts, best mean 0.315"));
    assert!(seen[2].0.trim_end().ends_with("first"));
    assert_eq!(*port.temperatures.lock().unwrap(), [0.0, 0.8, 0.0]);

    let ledger: ScriptLedger =
        serde_json::from_slice(&std::fs::read(run.dir.join("ledger.json")).unwrap()).unwrap();
    let rows = &ledger.candidates[1..];
    let approaches: Vec<Option<&str>> = rows.iter().map(|c| c.approach.as_deref()).collect();
    assert_eq!(
        approaches,
        [Some("Cheaper loss"), Some("cheaper-loss!"), None]
    );
    assert_eq!(rows[0].differs_from_previous, None);
    assert_eq!(
        rows[1].differs_from_previous.as_deref(),
        Some("a smaller prefix")
    );
    let temperatures: Vec<Option<f64>> = rows.iter().map(|c| c.temperature).collect();
    assert_eq!(temperatures, [None, Some(0.8), None]);

    let ideated: Vec<Option<String>> = trace
        .records()
        .iter()
        .filter_map(|r| match &r.event {
            LoopEvent::Ideated { approach, .. } => Some(approach.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        ideated,
        [
            Some("Cheaper loss".to_owned()),
            Some("cheaper-loss!".to_owned()),
            None
        ]
    );
}

#[test]
fn a_blank_differs_statement_is_not_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = RunLocation {
        id: "run-1".into(),
        dir: dir.path().join("run"),
    };
    let port = Scripted::new(vec![labelled("a", Some(" "), &loss("0.3"))]);
    let policy = LoopPolicy {
        max_epochs: 1,
        proposals_per_epoch: 1,
        ..LoopPolicy::default()
    };
    let mut trace = VecTrace::new(ManualClock::new());
    run_script_loop(&s, &supervisor(port), &policy, &run, &mut trace).unwrap();
    let ledger: ScriptLedger =
        serde_json::from_slice(&std::fs::read(run.dir.join("ledger.json")).unwrap()).unwrap();
    assert_eq!(ledger.candidates[1].differs_from_previous, None);
}

#[test]
fn refuses_blank_directives_and_unusable_temperatures_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let s = subject(dir.path(), &loss("0.5"));
    let run = RunLocation {
        id: "run-1".into(),
        dir: dir.path().join("run"),
    };
    let sup = supervisor(Scripted::new(vec![]));
    for slot in [
        SlotDirective::new(" \n"),
        SlotDirective::new("hot").with_temperature(2.5),
        SlotDirective::new("cold").with_temperature(-0.5),
        SlotDirective::new("nan").with_temperature(f64::NAN),
    ] {
        let policy = LoopPolicy {
            slots: vec![SlotDirective::new("fine"), slot],
            ..LoopPolicy::default()
        };
        let mut trace = VecTrace::new(ManualClock::new());
        assert_eq!(
            run_script_loop(&s, &sup, &policy, &run, &mut trace).unwrap_err(),
            Error::InvalidScriptSubject
        );
        assert!(trace.records().is_empty());
    }
    assert!(!run.dir.exists());
}
