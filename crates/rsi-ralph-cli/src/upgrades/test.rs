//! The upgrade log: wire forms, the text notation, the run-start diff and
//! the Markdown rendering.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use std::collections::BTreeMap;

use serde_json::json;

use super::*;
use crate::{
    manifest::{self, Loop},
    store::{RunRecord, RunStatus, write_json},
};

fn assistant_and_operator() -> Upgrade {
    Upgrade {
        id: "u-20261025-110000".into(),
        recorded_at: 1_792_925_000,
        title: "Keep every seat working".into(),
        contributors: vec![
            Contributor {
                author: Author::Assistant {
                    name: "claude-code".into(),
                    model: Some("claude-opus-5-5".into()),
                },
                role: Role::Diagnosed,
                rationale: Some("The lead ends each seat's work at its first report.".into()),
            },
            Contributor {
                author: Author::Operator {
                    name: "steven".into(),
                },
                role: Role::Approved,
                rationale: Some("Idle seats waste the epoch.".into()),
            },
        ],
        applies_from: AppliesFrom {
            run: Some("20261025-113628".into()),
            epoch: None,
        },
        evidence: vec![Evidence {
            metric: Some("seats_busy_mean".into()),
            value: Some(7.2),
            run: Some("erdos-779/20261025-103058".into()),
            ..Evidence::default()
        }],
        diagnosis: Some("The lead prompt ended each seat's work at its first report.".into()),
        change: Change {
            summary: "The lead answers every report with the next assignment.".into(),
            files: vec![FileChange {
                path: "hive/constitution.json".into(),
                keys: vec!["max_turns".into()],
                before: Some("aa".into()),
                after: Some("bb".into()),
            }],
            diff: None,
        },
        expected: vec![Prediction {
            metric: "seats_busy_mean".into(),
            op: Comparison::Ge,
            value: 10.0,
            note: Some("was 7.2".into()),
        }],
    }
}

#[test]
fn an_upgrade_with_two_contributors_has_a_pinned_wire_form() {
    let entry = Entry::Upgrade(assistant_and_operator());
    let wire = json!({
        "kind": "upgrade",
        "id": "u-20261025-110000",
        "recorded_at": 1_792_925_000_u64,
        "title": "Keep every seat working",
        "contributors": [
            {
                "author": { "kind": "assistant", "name": "claude-code", "model": "claude-opus-5-5" },
                "role": "diagnosed",
                "rationale": "The lead ends each seat's work at its first report."
            },
            {
                "author": { "kind": "operator", "name": "steven" },
                "role": "approved",
                "rationale": "Idle seats waste the epoch."
            }
        ],
        "applies_from": { "run": "20261025-113628" },
        "evidence": [
            { "metric": "seats_busy_mean", "value": 7.2, "run": "erdos-779/20261025-103058" }
        ],
        "diagnosis": "The lead prompt ended each seat's work at its first report.",
        "change": {
            "summary": "The lead answers every report with the next assignment.",
            "files": [
                { "path": "hive/constitution.json", "keys": ["max_turns"], "before": "aa", "after": "bb" }
            ]
        },
        "expected": [
            { "metric": "seats_busy_mean", "op": "ge", "value": 10.0, "note": "was 7.2" }
        ]
    });
    assert_eq!(serde_json::to_value(&entry).unwrap(), wire);
    assert_eq!(serde_json::from_value::<Entry>(wire).unwrap(), entry);
}

#[test]
fn hive_and_loop_authors_and_bare_records_have_pinned_wire_forms() {
    let hive = Author::Hive {
        seat: "lead".into(),
    };
    assert_eq!(
        serde_json::to_value(&hive).unwrap(),
        json!({ "kind": "hive", "seat": "lead" })
    );
    let mechanism = Author::Loop {
        mechanism: "run_start_diff".into(),
    };
    assert_eq!(
        serde_json::to_value(&mechanism).unwrap(),
        json!({ "kind": "loop", "mechanism": "run_start_diff" })
    );
    let bare = json!({
        "kind": "upgrade", "id": "u-1", "recorded_at": 1, "title": "t",
        "contributors": [ { "author": { "kind": "operator", "name": "o" }, "role": "authored" } ],
        "change": { "summary": "s" }
    });
    let entry: Entry = serde_json::from_value(bare.clone()).unwrap();
    assert_eq!(serde_json::to_value(&entry).unwrap(), bare);
}

#[test]
fn outcomes_and_applied_entries_have_pinned_wire_forms() {
    let outcome = Entry::Outcome(Outcome {
        upgrade: "u-1".into(),
        recorded_at: 5,
        run: "r2".into(),
        verdict: Verdict::Mixed,
        measured: vec![Evidence {
            metric: Some("m".into()),
            value: Some(1.5),
            ..Evidence::default()
        }],
        contributors: Vec::new(),
    });
    assert_eq!(
        serde_json::to_value(&outcome).unwrap(),
        json!({
            "kind": "outcome", "upgrade": "u-1", "recorded_at": 5, "run": "r2",
            "verdict": "mixed", "measured": [ { "metric": "m", "value": 1.5 } ]
        })
    );
    let applied = Entry::Applied(Applied {
        run: "r2".into(),
        recorded_at: 6,
        previous_run: Some("r1".into()),
        upgrades: vec!["u-1".into()],
        files: vec![FileChange {
            path: "brief.md".into(),
            after: Some("cc".into()),
            ..FileChange::default()
        }],
        unexplained: vec!["brief.md".into()],
    });
    assert_eq!(
        serde_json::to_value(&applied).unwrap(),
        json!({
            "kind": "applied", "run": "r2", "recorded_at": 6, "previous_run": "r1",
            "upgrades": ["u-1"], "files": [ { "path": "brief.md", "after": "cc" } ],
            "unexplained": ["brief.md"]
        })
    );
}

#[test]
fn a_log_line_that_does_not_decode_is_named_by_number() {
    let err = parse("\n{\"kind\":\"nonsense\"}\n", Path::new("x/upgrades.jsonl")).unwrap_err();
    assert!(
        matches!(&err, Error::Store { path, message } if path.ends_with("upgrades.jsonl") && message.starts_with("line 2:")),
        "{err}"
    );
}

#[test]
fn append_then_read_round_trips_and_a_missing_log_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join(LOG);
    assert!(read(&path).unwrap().is_empty());
    let entry = Entry::Upgrade(assistant_and_operator());
    append(&path, &entry).unwrap();
    append(&path, &entry).unwrap();
    assert_eq!(read(&path).unwrap(), vec![entry.clone(), entry]);
}

#[test]
fn fresh_ids_skip_the_ones_taken() {
    let mut taken = assistant_and_operator();
    taken.id = format!("u-{}", crate::store::timestamp_id(0));
    let entries = vec![Entry::Upgrade(taken.clone())];
    assert_eq!(next_id(&[], 0), "u-19700101-000000");
    assert_eq!(next_id(&entries, 0), "u-19700101-000000-2");
}

#[test]
fn outcomes_are_confirmed_refuted_or_mixed_by_the_predictions_measured() {
    let p = |metric: &str, op, value| Prediction {
        metric: metric.into(),
        op,
        value,
        note: None,
    };
    let m = |metric: &str, value| Evidence {
        metric: Some(metric.into()),
        value: Some(value),
        ..Evidence::default()
    };
    let expected = [p("a", Comparison::Ge, 10.0), p("b", Comparison::Lt, 2.0)];
    assert_eq!(
        evaluate(&expected, &[m("a", 10.0), m("b", 1.0)]),
        Some(Verdict::Confirmed)
    );
    assert_eq!(
        evaluate(&expected, &[m("a", 9.0), m("b", 3.0)]),
        Some(Verdict::Refuted)
    );
    assert_eq!(
        evaluate(&expected, &[m("a", 11.0), m("b", 3.0)]),
        Some(Verdict::Mixed)
    );
    assert_eq!(evaluate(&expected, &[m("c", 1.0)]), None);
    assert!(Comparison::Gt.holds(2.0, 1.0) && !Comparison::Gt.holds(1.0, 1.0));
    assert!(Comparison::Le.holds(1.0, 1.0) && Comparison::Eq.holds(1.0, 1.0));
}

#[test]
fn the_notation_parses_every_kind_of_author_and_role() {
    let c = notation::contributor("assistant:claude-code@claude-opus-5-5=diagnosed").unwrap();
    assert_eq!(
        c.author,
        Author::Assistant {
            name: "claude-code".into(),
            model: Some("claude-opus-5-5".into())
        }
    );
    assert_eq!(c.role, Role::Diagnosed);
    let c = notation::contributor("operator:steven").unwrap();
    assert_eq!(
        (c.author, c.role),
        (
            Author::Operator {
                name: "steven".into()
            },
            Role::Authored
        )
    );
    assert_eq!(
        notation::contributor("hive:lead=proposed").unwrap().author,
        Author::Hive {
            seat: "lead".into()
        }
    );
    assert_eq!(
        notation::contributor("loop:run_start_diff=detected")
            .unwrap()
            .author,
        Author::Loop {
            mechanism: "run_start_diff".into()
        }
    );
    for bad in [
        "steven",
        "robot:x",
        "operator:",
        "operator:steven@gpt",
        "assistant:x@",
        "operator:steven=boss",
    ] {
        assert!(
            matches!(notation::contributor(bad), Err(Error::Field { .. })),
            "{bad}"
        );
    }
}

#[test]
fn the_notation_parses_metrics_predictions_files_and_verdicts() {
    let e = notation::metric("seats_busy_mean=7.2@erdos-779/r9").unwrap();
    assert_eq!(e.metric.as_deref(), Some("seats_busy_mean"));
    assert_eq!(e.value, Some(7.2));
    assert_eq!(e.run.as_deref(), Some("erdos-779/r9"));
    for bad in ["=1", "m", "m=x", "m=inf", "m=1@"] {
        assert!(notation::metric(bad).is_err(), "{bad}");
    }
    for (text, op, value) in [
        ("busy>=10", Comparison::Ge, 10.0),
        ("p90_s <= 30", Comparison::Le, 30.0),
        ("x>1", Comparison::Gt, 1.0),
        ("x<1", Comparison::Lt, 1.0),
        ("x=0.5", Comparison::Eq, 0.5),
    ] {
        let p = notation::prediction(text).unwrap();
        assert_eq!((p.op, p.value), (op, value), "{text}");
    }
    assert_eq!(notation::prediction("p90_s <= 30").unwrap().metric, "p90_s");
    for bad in ["x", ">=1", "x>=y"] {
        assert!(notation::prediction(bad).is_err(), "{bad}");
    }
    let f = notation::file("./hive/constitution.json#max_turns, max_width").unwrap();
    assert_eq!(f.path, "hive/constitution.json");
    assert_eq!(f.keys, ["max_turns", "max_width"]);
    for bad in ["", "/etc/passwd", "hive/../../x"] {
        assert!(notation::file(bad).is_err(), "{bad}");
    }
    assert_eq!(notation::verdict("refuted").unwrap(), Verdict::Refuted);
    assert!(notation::verdict("maybe").is_err());
}

fn files(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn compare_reports_changed_added_and_removed_files_but_skips_excluded_ones() {
    let before = files(&[
        ("brief.md", "a"),
        ("hive/seats/lead.md", "x"),
        ("old.md", "o"),
        ("subject/v.txt", "1"),
        (LOG, "{}"),
    ]);
    let after = files(&[
        ("brief.md", "a"),
        ("hive/seats/lead.md", "y"),
        ("new.md", "n"),
        ("subject/v.txt", "2"),
        (LOG, "{}\n{}"),
    ]);
    let excluded = [LOG.to_owned(), "subject/".to_owned()];
    let changes = drift::compare(&before, &after, &excluded);
    let paths: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["hive/seats/lead.md", "new.md", "old.md"]);
    assert_eq!(changes[0].before.as_deref(), Some(digest("x").as_str()));
    assert_eq!(changes[0].after.as_deref(), Some(digest("y").as_str()));
    assert_eq!(changes[1].before, None);
    assert_eq!(changes[2].after, None);
    assert!(drift::compare(&before, &before, &excluded).is_empty());
    let summary = drift::summary(&changes, &before, &after);
    assert_eq!(
        summary,
        "hive/seats/lead.md: modified (1 -> 1 lines)\nnew.md: added (1 lines)\nold.md: removed (1 lines)"
    );
    assert!(drift::covers("hive/seats/", "hive/seats/lead.md"));
    assert!(!drift::covers("hive/seats", "hive/seats/lead.md"));
}

/// The graph-colouring example loop, its files, and a store in `root`.
fn example(root: &Path) -> (BTreeMap<String, String>, Loop, Store) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../loops/graph-coloring");
    let files = manifest::load_dir(&dir).unwrap();
    let parsed = manifest::parse(&files).unwrap();
    (files, parsed, Store::new(root))
}

/// Store a finished run of `name` started at `at`, with `files` frozen.
fn stored(store: &Store, name: &str, id: &str, at: u64, files: &BTreeMap<String, String>) {
    let run = store.create_run(name, Some(id), files).unwrap();
    finish(&run, at);
}

/// Mark a stored run finished, started at `at`.
fn finish(run: &crate::store::RunDir, at: u64) {
    let (name, id) = (run.loop_name.as_str(), run.id.as_str());
    write_json(
        &run.record_path(),
        &RunRecord {
            id: id.into(),
            r#loop: name.into(),
            status: RunStatus::Finished,
            pid: 0,
            started_at: at,
            finished_at: Some(at + 1),
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
}

fn run_log(store: &Store, name: &str, id: &str) -> Vec<Entry> {
    read(&store.runs_root().join(name).join(id).join(LOG)).unwrap()
}

#[test]
fn a_loops_first_run_records_that_it_has_nothing_to_compare_with() {
    let home = tempfile::tempdir().unwrap();
    let (files, parsed, store) = example(home.path());
    let name = parsed.manifest.name.clone();
    let run = store.create_run(&name, Some("r1"), &files).unwrap();
    let warning = drift::check_run_start(&store, &run, &files, &parsed, 7).unwrap();
    assert_eq!(warning, None);
    let log = read(&run.path.join(LOG)).unwrap();
    assert!(
        matches!(log.as_slice(), [Entry::Applied(a)] if a.previous_run.is_none() && a.files.is_empty())
    );
    let md = std::fs::read_to_string(run.path.join(RENDERED)).unwrap();
    assert!(md.contains("## Run r1 started"), "{md}");
    assert!(md.contains("The loop's first stored run."), "{md}");
}

#[test]
fn an_unchanged_hive_records_no_change_and_no_warning() {
    let home = tempfile::tempdir().unwrap();
    let (files, parsed, store) = example(home.path());
    let name = parsed.manifest.name.clone();
    stored(&store, &name, "r1", 10, &files);
    let run = store.create_run(&name, Some("r2"), &files).unwrap();
    assert_eq!(
        drift::check_run_start(&store, &run, &files, &parsed, 20).unwrap(),
        None
    );
    let log = run_log(&store, &name, "r2");
    assert!(
        matches!(log.as_slice(), [Entry::Applied(a)] if a.previous_run.as_deref() == Some("r1") && a.files.is_empty() && a.unexplained.is_empty()),
        "{log:?}"
    );
}

#[test]
fn a_hive_change_with_no_upgrade_record_is_recorded_as_unexplained() {
    let home = tempfile::tempdir().unwrap();
    let (files, parsed, store) = example(home.path());
    let name = parsed.manifest.name.clone();
    stored(&store, &name, "r1", 10, &files);
    let mut changed = files.clone();
    let seat = changed
        .keys()
        .find(|k| k.starts_with("hive/") && k.ends_with(".md"))
        .unwrap()
        .clone();
    changed.get_mut(&seat).unwrap().push_str("\nWork harder.\n");
    let run = store.create_run(&name, Some("r2"), &changed).unwrap();
    let warning = drift::check_run_start(&store, &run, &changed, &parsed, 20)
        .unwrap()
        .unwrap();
    assert_eq!(
        warning,
        format!("changed since run r1 with no upgrade record: {seat}")
    );
    let log = run_log(&store, &name, "r2");
    let [Entry::Applied(applied), Entry::Upgrade(unexplained)] = log.as_slice() else {
        panic!("expected applied then upgrade: {log:?}");
    };
    assert_eq!(applied.unexplained, [seat.clone()]);
    assert_eq!(
        unexplained.contributors[0].author,
        Author::Loop {
            mechanism: drift::MECHANISM.into()
        }
    );
    assert_eq!(unexplained.contributors[0].role, Role::Detected);
    assert_eq!(unexplained.applies_from.run.as_deref(), Some("r2"));
    assert_eq!(unexplained.change.files[0].path, seat);
    assert_eq!(
        unexplained.change.files[0].before.as_deref(),
        Some(digest(&files[&seat]).as_str())
    );
    assert!(
        unexplained
            .change
            .diff
            .as_deref()
            .unwrap()
            .contains("modified")
    );
    let md = std::fs::read_to_string(run.path.join(RENDERED)).unwrap();
    assert!(md.contains("## Unexplained change since run r1"), "{md}");
    assert!(
        md.contains(&format!("No upgrade record explains `{seat}`.")),
        "{md}"
    );
}

#[test]
fn a_hive_change_named_by_a_new_upgrade_is_explained_and_carried() {
    let home = tempfile::tempdir().unwrap();
    let (files, parsed, store) = example(home.path());
    let name = parsed.manifest.name.clone();
    stored(&store, &name, "r1", 10, &files);
    let mut changed = files.clone();
    changed.insert("hive/context/new.md".into(), "more context".into());
    let mut upgrade = assistant_and_operator();
    upgrade.applies_from = AppliesFrom::default();
    upgrade.change.files = vec![FileChange {
        path: "hive/context/".into(),
        ..FileChange::default()
    }];
    let line = serde_json::to_string(&Entry::Upgrade(upgrade.clone())).unwrap();
    changed.insert(LOG.into(), line + "\n");
    let run = store.create_run(&name, Some("r2"), &changed).unwrap();
    assert_eq!(
        drift::check_run_start(&store, &run, &changed, &parsed, 20).unwrap(),
        None
    );
    let log = run_log(&store, &name, "r2");
    let [Entry::Applied(applied)] = log.as_slice() else {
        panic!("expected one applied entry: {log:?}");
    };
    assert_eq!(applied.upgrades, [upgrade.id.clone()]);
    assert_eq!(applied.files.len(), 1);
    assert!(applied.unexplained.is_empty());
    // The run after carries nothing new: r2's frozen log already held it.
    finish(&run, 20);
    let run = store.create_run(&name, Some("r3"), &changed).unwrap();
    drift::check_run_start(&store, &run, &changed, &parsed, 30).unwrap();
    let log = run_log(&store, &name, "r3");
    assert!(matches!(log.as_slice(), [Entry::Applied(a)] if a.upgrades.is_empty()));
    let md = std::fs::read_to_string(run.path.join(RENDERED)).unwrap();
    assert!(
        md.contains(&format!("`{}` · from run r2", upgrade.id)),
        "{md}"
    );
}

#[test]
fn a_dir_subject_inside_the_loop_is_not_part_of_the_diff() {
    let home = tempfile::tempdir().unwrap();
    let (_, parsed, _) = example(home.path());
    assert_eq!(
        drift::excluded(&parsed),
        [LOG.to_owned(), "subject/".to_owned()]
    );
}

#[test]
fn collect_reads_the_loop_log_then_each_run_oldest_first() {
    let home = tempfile::tempdir().unwrap();
    let (files, parsed, store) = example(home.path());
    let name = parsed.manifest.name.clone();
    stored(&store, &name, "b", 20, &files);
    stored(&store, &name, "a", 10, &files);
    let entry = |run: &str| {
        Entry::Applied(Applied {
            run: run.into(),
            recorded_at: 0,
            previous_run: None,
            upgrades: Vec::new(),
            files: Vec::new(),
            unexplained: Vec::new(),
        })
    };
    for id in ["a", "b"] {
        append(
            &store.runs_root().join(&name).join(id).join(LOG),
            &entry(id),
        )
        .unwrap();
    }
    let loop_log = vec![Entry::Upgrade(assistant_and_operator())];
    let all = collect(&store, &name, loop_log.clone()).unwrap();
    assert_eq!(all, [loop_log[0].clone(), entry("a"), entry("b")]);
    let hook = store.runs_root().join(&name).join("a");
    let run = crate::store::RunDir {
        path: hook.clone(),
        id: "a".into(),
        loop_name: name.clone(),
    };
    record(&run, assistant_and_operator()).unwrap();
    assert_eq!(read(&hook.join(LOG)).unwrap().len(), 2);
}

#[test]
fn markdown_reads_like_a_hand_kept_upgrades_file() {
    let upgrade = assistant_and_operator();
    let outcome = Outcome {
        upgrade: upgrade.id.clone(),
        recorded_at: upgrade.recorded_at + 10,
        run: "20261025-113628".into(),
        verdict: Verdict::Confirmed,
        measured: vec![
            Evidence {
                metric: Some("seats_busy_mean".into()),
                value: Some(10.4),
                ..Evidence::default()
            },
            Evidence {
                note: Some("the scout worked to the end".into()),
                claim: Some("c-17".into()),
                ..Evidence::default()
            },
        ],
        contributors: vec![Contributor {
            author: Author::Operator {
                name: "steven".into(),
            },
            role: Role::Authored,
            rationale: Some("All twelve seats stayed busy.".into()),
        }],
    };
    let entries = vec![Entry::Outcome(outcome), Entry::Upgrade(upgrade)];
    let md = render::markdown("erdos-779", &entries);
    let expected = "# Upgrades: erdos-779

## Keep every seat working

`u-20261025-110000` · from run 20261025-113628 · by assistant claude-code (claude-opus-5-5), diagnosed; operator steven, approved

**Saw**

- `seats_busy_mean` = 7.2 (run erdos-779/20261025-103058)

**Diagnosis.** The lead prompt ended each seat's work at its first report.

**Change.** The lead answers every report with the next assignment.

- `hive/constitution.json` keys max_turns (aa -> bb)

**Why**

- assistant claude-code (claude-opus-5-5): The lead ends each seat's work at its first report.
- operator steven: Idle seats waste the epoch.

**Expected**

- `seats_busy_mean` >= 10 (was 7.2)

**Outcome in run 20261025-113628: confirmed** (by operator steven, authored)

- `seats_busy_mean` = 10.4: expected >= 10, held
- the scout worked to the end (claim c-17)
- operator steven: All twelve seats stayed busy.
";
    assert_eq!(md, expected);
    assert_eq!(
        render::markdown("empty", &[]),
        "# Upgrades: empty\n\nNo upgrades recorded.\n"
    );
}

#[test]
fn markdown_says_when_an_upgrade_waits_for_the_next_run_or_a_prediction_was_not_measured() {
    let mut upgrade = assistant_and_operator();
    upgrade.applies_from = AppliesFrom {
        run: None,
        epoch: Some(2),
    };
    let outcome = Outcome {
        upgrade: upgrade.id.clone(),
        recorded_at: 0,
        run: "r2".into(),
        verdict: Verdict::Refuted,
        measured: Vec::new(),
        contributors: Vec::new(),
    };
    let applied = Applied {
        run: "r2".into(),
        recorded_at: upgrade.recorded_at + 1,
        previous_run: Some("r1".into()),
        upgrades: Vec::new(),
        files: vec![
            FileChange {
                path: "a.md".into(),
                ..FileChange::default()
            },
            FileChange {
                path: "b.md".into(),
                before: Some("0123456789".into()),
                ..FileChange::default()
            },
        ],
        unexplained: Vec::new(),
    };
    let md = render::markdown(
        "l",
        &[
            Entry::Upgrade(upgrade),
            Entry::Outcome(outcome),
            Entry::Applied(applied),
        ],
    );
    assert!(md.contains("· from the next run, epoch 2 ·"), "{md}");
    assert!(
        md.contains("**Outcome in run r2: refuted**\n\n- `seats_busy_mean`: not measured"),
        "{md}"
    );
    assert!(
        md.contains("2 files changed since run r1:\n\n- `a.md`\n- `b.md` (01234567 -> none)"),
        "{md}"
    );
}
