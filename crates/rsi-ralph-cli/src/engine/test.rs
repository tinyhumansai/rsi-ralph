//! The epoch loop end to end with a scripted hive and shell scorers.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::{manifest, subject::copy_tree};
use std::fs;
use rsi_ralph_gateway::{CallMark, Refusal, UpstreamCalls};

/// What a seat writes in an epoch, as `path:content`, or nothing.
type Plan = Box<dyn Fn(u32, &str) -> Option<String>>;

/// A hive stand-in that writes the next value for each seat.
struct Scripted {
    seats: Vec<String>,
    /// Value each seat writes per epoch; `None` leaves the candidate alone.
    plan: Plan,
    briefs: Vec<String>,
}

impl Proposer for Scripted {
    fn editing_seats(&self) -> Vec<String> {
        self.seats.clone()
    }

    fn propose(&mut self, request: &ProposalRequest<'_>) -> Result<ProposalReport> {
        self.briefs.push(request.brief.to_owned());
        for (seat, checkout) in request.candidates {
            if let Some(text) = (self.plan)(request.epoch, seat) {
                let (path, body) = text.split_once(':').unwrap();
                fs::write(checkout.path.join(path), body).unwrap();
            }
        }
        Ok(ProposalReport {
            summary: Some("done".into()),
            seats: BTreeMap::new(),
        })
    }
}

struct Ideas(Vec<(u32, bool)>);

impl Ideator for Ideas {
    fn ideas(&mut self, epoch: u32, brief: &str, _dir: &Path) -> Result<Vec<String>> {
        assert!(brief.contains("Lower"));
        Ok(vec![format!("idea for epoch {epoch}")])
    }

    fn outcome(&mut self, epoch: u32, promoted: bool) -> Result<()> {
        self.0.push((epoch, promoted));
        Ok(())
    }
}

struct Spend(f64);

impl Meter for Spend {
    fn spent_usd(&self) -> f64 {
        0.5
    }

    fn remaining_usd(&self) -> Option<f64> {
        Some(self.0)
    }
}

/// A loop over `value.txt` whose score is the number in it (lower is better).
fn fixture(root: &Path, edit: impl FnOnce(&mut serde_json::Value)) -> (PathBuf, Loop) {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../loops/graph-coloring");
    let dir = root.join("loop");
    copy_tree(&example, &dir).unwrap();
    fs::write(dir.join("subject/value.txt"), "9").unwrap();
    fs::write(
        dir.join("scorers/m.sh"),
        "echo \"METRIC m=$(cat \"$RSI_CANDIDATE/value.txt\")\"\n",
    )
    .unwrap();
    fs::write(dir.join("brief.md"), "Lower the value.\n\n{{incumbent}}\n").unwrap();
    let mut m = serde_json::json!({
        "$schema": manifest::SCHEMA, "version": 1, "name": "value",
        "subject": { "kind": "dir", "path": "subject", "editable": ["value.txt"] },
        "brief": "brief.md",
        "hive": { "package": "hive", "lead": "lead" },
        "score": {
            "scorers": [ { "id": "m", "run": "/bin/sh scorers/m.sh" } ],
            "decide": { "on": "m", "test": "seeds", "seeds": [1, 2], "direction": "minimize", "target": 2 }
        },
        "epochs": 4, "patience": 2
    });
    edit(&mut m);
    fs::write(dir.join("rsi.json"), m.to_string()).unwrap();
    let parsed = manifest::parse(&manifest::load_dir(&dir).unwrap()).unwrap();
    (dir, parsed)
}

fn engine<'a>(
    parsed: &'a Loop,
    dir: &Path,
    store: &'a Store,
    run: &str,
    proposer: &'a mut dyn Proposer,
) -> Engine<'a> {
    Engine {
        parsed,
        loop_dir: dir.to_path_buf(),
        store,
        run: store
            .create_run("value", Some(run), &BTreeMap::new())
            .unwrap(),
        proposer,
        ideator: None,
        judges: Judges::new(),
        meter: None,
        model_url: None,
        inherited: vec![("PATH".into(), "/usr/bin:/bin".into())],
        secrets: vec!["sk-secret-value".into()],
        epochs: None,
    }
}

fn events(store: &Store, run: &str) -> Vec<String> {
    fs::read_to_string(
        store
            .runs_root()
            .join("value")
            .join(run)
            .join("events.jsonl"),
    )
    .unwrap()
    .lines()
    .map(|l| {
        serde_json::from_str::<serde_json::Value>(l).unwrap()["event"]["kind"]
            .as_str()
            .unwrap()
            .to_owned()
    })
    .collect()
}

#[test]
fn improves_until_the_goal_and_records_everything() {
    let root = tempfile::tempdir().unwrap();
    let (dir, parsed) = fixture(root.path(), |_| {});
    let store = Store::new(root.path().join("home"));
    let mut hive = Scripted {
        seats: vec!["a".into(), "b".into()],
        // Seat a lowers by 3 each epoch, seat b by 1, so a always wins.
        plan: Box::new(|epoch, seat| {
            let step = if seat == "a" { 3 } else { 1 };
            Some(format!("value.txt:{}", 9 - 3 * (epoch - 1) - step))
        }),
        briefs: Vec::new(),
    };
    let mut ideas = Ideas(Vec::new());
    let mut e = engine(&parsed, &dir, &store, "r1", &mut hive);
    e.ideator = Some(&mut ideas);
    let summary = e.run().unwrap();
    assert_eq!(summary.reason, StopReason::GoalReached);
    assert_eq!(summary.score, Some(0.0));
    assert_eq!(summary.promotions, 3);
    assert_eq!(
        fs::read_to_string(summary.checkout.join("value.txt")).unwrap(),
        "0"
    );
    assert_eq!(ideas.0, vec![(1, true), (2, true), (3, true)]);
    assert!(hive.briefs[1].contains("`m` (deciding): 6.0000"));
    assert!(hive.briefs[0].contains("- idea for epoch 1"));

    let (run, record) = store.find_run("value/r1").unwrap();
    assert_eq!(record.status, RunStatus::Finished);
    assert_eq!(record.stop_reason, Some(StopReason::GoalReached));
    assert_eq!(record.epochs, 3);
    let ledger: Ledger = store::read_json(&run.ledger_path()).unwrap();
    assert_eq!(ledger.incumbents.len(), 4);
    assert_eq!(ledger.candidates.len(), 6);
    assert_eq!(ledger.candidates[0].status, CandidateStatus::Promoted);
    assert_eq!(ledger.candidates[1].status, CandidateStatus::Scored);
    assert_eq!(ledger.epochs[0].promoted.as_deref(), Some("e1-a"));
    assert!(run.path.join("epoch-1/brief.md").exists());
    assert!(run.path.join("epoch-1/steps/e1-a/change.diff").exists());

    let kinds = events(&store, "r1");
    assert_eq!(kinds.first().map(String::as_str), Some("run_started"));
    assert_eq!(kinds.last().map(String::as_str), Some("run_finished"));
    for kind in [
        "epoch_started",
        "observed",
        "ideated",
        "executed",
        "measured",
        "scored",
        "improved",
        "epoch_finished",
    ] {
        assert!(kinds.iter().any(|k| k == kind), "missing {kind}");
    }

    // A second run of the same revision reuses the cached baseline.
    let mut idle = Scripted {
        seats: vec!["a".into()],
        plan: Box::new(|_, _| None),
        briefs: Vec::new(),
    };
    let summary = engine(&parsed, &dir, &store, "r2", &mut idle)
        .run()
        .unwrap();
    assert_eq!(summary.reason, StopReason::Stalled);
    assert!(!store.runs_root().join("value/r2/epoch-0").exists());
    let ledger: Ledger = store::read_json(&store.runs_root().join("value/r2/ledger.json")).unwrap();
    assert!(
        ledger
            .candidates
            .iter()
            .all(|c| c.status == CandidateStatus::Empty)
    );
}

#[test]
fn gates_reject_out_of_surface_and_failing_candidates() {
    let root = tempfile::tempdir().unwrap();
    let (dir, parsed) = fixture(root.path(), |m| {
        m["gate"] = serde_json::json!([{ "run": "/bin/sh -c exit_1", "cwd": "candidate" }]);
        m["epochs"] = serde_json::json!(1);
    });
    let store = Store::new(root.path().join("home"));
    let mut hive = Scripted {
        seats: vec!["a".into(), "b".into()],
        plan: Box::new(|_, seat| {
            Some(if seat == "a" {
                "other.txt:x".into()
            } else {
                "value.txt:1".into()
            })
        }),
        briefs: Vec::new(),
    };
    let summary = engine(&parsed, &dir, &store, "g", &mut hive).run().unwrap();
    assert_eq!(summary.reason, StopReason::MaxEpochs);
    let ledger: Ledger = store::read_json(&store.runs_root().join("value/g/ledger.json")).unwrap();
    assert_eq!(ledger.candidates[0].status, CandidateStatus::Gated);
    assert_eq!(
        ledger.candidates[0].error.as_deref(),
        Some("other.txt is outside the editable surface")
    );
    assert_eq!(ledger.candidates[1].status, CandidateStatus::Gated);
    assert_eq!(ledger.candidates[1].gates.len(), 2);
}

#[test]
fn a_worse_or_broken_candidate_is_not_promoted() {
    let root = tempfile::tempdir().unwrap();
    let (dir, parsed) = fixture(root.path(), |m| m["epochs"] = serde_json::json!(1));
    let store = Store::new(root.path().join("home"));
    let mut hive = Scripted {
        seats: vec!["a".into(), "b".into()],
        plan: Box::new(|_, seat| {
            Some(if seat == "a" {
                "value.txt:20".into()
            } else {
                "value.txt:oops".into()
            })
        }),
        briefs: Vec::new(),
    };
    let summary = engine(&parsed, &dir, &store, "w", &mut hive).run().unwrap();
    assert_eq!(summary.promotions, 0);
    let ledger: Ledger = store::read_json(&store.runs_root().join("value/w/ledger.json")).unwrap();
    let verdict = ledger.candidates[0].verdict.as_ref().unwrap();
    assert_eq!(verdict.decision, ScoreDecision::Reject);
    assert!(
        ledger.candidates[1]
            .verdict
            .as_ref()
            .unwrap()
            .reason
            .starts_with("seed 1 failed")
    );
    let entries = history(&ledger);
    assert_eq!(entries[0].outcome, "rejected");
}

#[test]
fn stops_on_budget_before_an_epoch() {
    let root = tempfile::tempdir().unwrap();
    let (dir, parsed) = fixture(root.path(), |m| {
        m["budget"] = serde_json::json!({ "score_reserve_usd": 1.0 });
    });
    let store = Store::new(root.path().join("home"));
    let mut hive = Scripted {
        seats: vec!["a".into()],
        plan: Box::new(|_, _| None),
        briefs: Vec::new(),
    };
    let spend = Spend(0.5);
    // A meter that cannot see upstream calls reports none.
    assert_eq!(
        spend.upstream_calls_since(spend.call_mark()),
        UpstreamCalls::default()
    );
    let mut e = engine(&parsed, &dir, &store, "b", &mut hive);
    e.meter = Some(&spend);
    let summary = e.run().unwrap();
    assert_eq!(summary.reason, StopReason::Budget);
    let (_, record) = store.find_run("value/b").unwrap();
    assert!((record.spend_usd - 0.5).abs() < 1e-12);
}

#[test]
fn an_unscorable_incumbent_fails_the_run_with_a_scrubbed_error() {
    let root = tempfile::tempdir().unwrap();
    let (dir, parsed) = fixture(root.path(), |_| {});
    fs::write(
        dir.join("scorers/m.sh"),
        "echo 'bad key sk-secret-value' >&2; exit 1\n",
    )
    .unwrap();
    let store = Store::new(root.path().join("home"));
    let mut hive = Scripted {
        seats: vec![],
        plan: Box::new(|_, _| None),
        briefs: Vec::new(),
    };
    let err = engine(&parsed, &dir, &store, "f", &mut hive)
        .run()
        .unwrap_err();
    assert!(matches!(err, Error::Step { .. }));
    let (_, record) = store.find_run("value/f").unwrap();
    assert_eq!(record.status, RunStatus::Failed);
    let message = record.error.unwrap();
    assert!(message.contains("***") && !message.contains("sk-secret-value"));
}

#[test]
fn an_incumbent_already_at_the_target_stops_at_once() {
    let root = tempfile::tempdir().unwrap();
    let (dir, parsed) = fixture(root.path(), |m| {
        m["score"]["decide"]["target"] = serde_json::json!(10);
    });
    let store = Store::new(root.path().join("home"));
    let mut hive = Scripted {
        seats: vec![],
        plan: Box::new(|_, _| None),
        briefs: Vec::new(),
    };
    let mut e = engine(&parsed, &dir, &store, "t", &mut hive);
    e.epochs = Some(1);
    assert_eq!(e.run().unwrap().reason, StopReason::GoalReached);
}

#[test]
fn the_incumbent_cache_keeps_scores_under_different_env_apart() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::new(root.path().join("home"));
    let baseline = |value: &str, run: &str| {
        let (dir, _) = fixture(root.path(), |m| {
            m["env"] = serde_json::json!({ "VALUE": value });
        });
        fs::write(dir.join("scorers/m.sh"), "echo \"METRIC m=$VALUE\"\n").unwrap();
        let parsed = manifest::parse(&manifest::load_dir(&dir).unwrap()).unwrap();
        let mut hive = Scripted {
            seats: vec![],
            plan: Box::new(|_, _| None),
            briefs: Vec::new(),
        };
        let mut e = engine(&parsed, &dir, &store, run, &mut hive);
        e.epochs = Some(1);
        e.run().unwrap().score
    };
    assert_eq!(baseline("7", "a"), Some(7.0));
    assert_eq!(baseline("5", "b"), Some(5.0));
}

/// Each upstream call in dispatch order, with its refusal if it had one;
/// shared by a hive that makes calls and the meter that reports them.
type Calls = std::rc::Rc<std::cell::RefCell<Vec<Option<Refusal>>>>;

/// A hive whose model calls go through a provider: each epoch it makes
/// `sent` calls, the last `refused` of which the provider refuses.
struct Provided {
    hive: Scripted,
    calls: Calls,
    sent: u64,
    refused: u64,
    refusal: Refusal,
}

impl Proposer for Provided {
    fn editing_seats(&self) -> Vec<String> {
        self.hive.editing_seats()
    }

    fn propose(&mut self, request: &ProposalRequest<'_>) -> Result<ProposalReport> {
        let mut calls = self.calls.borrow_mut();
        for i in 0..self.sent {
            let refused = i >= self.sent - self.refused;
            calls.push(refused.then(|| self.refusal.clone()));
        }
        drop(calls);
        self.hive.propose(request)
    }
}

/// A meter that reports the shared call outcomes and no spend.
struct Watched(Calls);

impl Meter for Watched {
    fn spent_usd(&self) -> f64 {
        0.0
    }

    fn remaining_usd(&self) -> Option<f64> {
        None
    }

    fn call_mark(&self) -> CallMark {
        CallMark(u64::try_from(self.0.borrow().len()).unwrap())
    }

    fn upstream_calls_since(&self, mark: CallMark) -> UpstreamCalls {
        let calls = self.0.borrow();
        let later = &calls[usize::try_from(mark.0).unwrap()..];
        let refusals: Vec<&Refusal> = later.iter().flatten().collect();
        UpstreamCalls {
            sent: u64::try_from(later.len()).unwrap(),
            refused: u64::try_from(refusals.len()).unwrap(),
            last_refusal: refusals.last().map(|r| (*r).clone()),
        }
    }
}

fn provided(sent: u64, refused: u64, plan: Plan, refusal: Refusal) -> (Provided, Watched) {
    // Calls made before the run, such as the retreat's, are not the hive's.
    let earlier = Refusal {
        status: 500,
        message: "before the hive".into(),
    };
    let calls = Calls::new(std::cell::RefCell::new(vec![None, Some(earlier)]));
    let hive = Provided {
        hive: Scripted {
            seats: vec!["a".into()],
            plan,
            briefs: Vec::new(),
        },
        calls: calls.clone(),
        sent,
        refused,
        refusal,
    };
    (hive, Watched(calls))
}

fn event_lines(store: &Store, run: &str) -> Vec<serde_json::Value> {
    fs::read_to_string(
        store
            .runs_root()
            .join("value")
            .join(run)
            .join("events.jsonl"),
    )
    .unwrap()
    .lines()
    .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()["event"].clone())
    .collect()
}

#[test]
fn a_provider_refusing_every_call_stops_the_run_with_its_reason() {
    let root = tempfile::tempdir().unwrap();
    let (dir, parsed) = fixture(root.path(), |_| {});
    let store = Store::new(root.path().join("home"));
    let (mut hive, meter) = provided(
        2,
        2,
        Box::new(|_, _| None),
        Refusal {
            status: 403,
            message: "Key limit exceeded (daily limit) for sk-secret-value".into(),
        },
    );
    let mut e = engine(&parsed, &dir, &store, "refused", &mut hive);
    e.meter = Some(&meter);
    let summary = e.run().unwrap();
    assert_eq!(summary.reason, StopReason::ProviderRefused);
    let expected = "provider refused 2 of 2 model calls \
                    (403: Key limit exceeded (daily limit) for ***)";
    assert_eq!(summary.error.as_deref(), Some(expected));

    let (_, record) = store.find_run("value/refused").unwrap();
    assert_eq!(record.status, RunStatus::Failed);
    assert_eq!(record.stop_reason, Some(StopReason::ProviderRefused));
    assert_eq!(record.epochs, 1, "the run stops after the refused epoch");
    assert_eq!(record.error.as_deref(), Some(expected));

    let events = event_lines(&store, "refused");
    let kinds: Vec<&str> = events.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(
        kinds[kinds.len() - 3..],
        ["provider_refused", "epoch_finished", "run_finished"]
    );
    let refused = &events[kinds.len() - 3];
    assert_eq!(refused["epoch"], 1);
    assert_eq!(refused["calls"], 2);
    assert_eq!(refused["refused"], 2);
    assert_eq!(refused["status"], 403);
    assert_eq!(
        refused["message"],
        "Key limit exceeded (daily limit) for ***"
    );
    assert_eq!(events.last().unwrap()["reason"], "provider_refused");
}

#[test]
fn partial_refusals_in_an_empty_epoch_are_reported_and_the_run_goes_on() {
    let root = tempfile::tempdir().unwrap();
    let (dir, parsed) = fixture(root.path(), |_| {});
    let store = Store::new(root.path().join("home"));
    let (mut hive, meter) = provided(
        3,
        1,
        Box::new(|_, _| None),
        Refusal {
            status: 429,
            message: "Rate limit exceeded".into(),
        },
    );
    let mut e = engine(&parsed, &dir, &store, "partial", &mut hive);
    e.meter = Some(&meter);
    let summary = e.run().unwrap();
    assert_eq!(summary.reason, StopReason::Stalled);
    assert_eq!(summary.error, None);
    let warnings = [
        "epoch 1: provider refused 1 of 3 model calls (429: Rate limit exceeded)",
        "epoch 2: provider refused 1 of 3 model calls (429: Rate limit exceeded)",
    ];
    assert_eq!(summary.warnings, warnings);

    let (_, record) = store.find_run("value/partial").unwrap();
    assert_eq!(record.status, RunStatus::Finished);
    assert_eq!(record.stop_reason, Some(StopReason::Stalled));
    assert_eq!(record.error, None);
    assert_eq!(record.warnings, warnings);
    let refusals: Vec<_> = event_lines(&store, "partial")
        .into_iter()
        .filter(|e| e["kind"] == "provider_refused")
        .collect();
    assert_eq!(refusals.len(), 2);
    assert_eq!(refusals[1]["epoch"], 2);
    assert_eq!(refusals[1]["status"], 429);
}

#[test]
fn refusals_in_an_epoch_that_changed_something_are_not_reported() {
    let root = tempfile::tempdir().unwrap();
    let (dir, parsed) = fixture(root.path(), |m| m["epochs"] = serde_json::json!(1));
    let store = Store::new(root.path().join("home"));
    let (mut hive, meter) = provided(
        3,
        1,
        Box::new(|_, _| Some("value.txt:5".into())),
        Refusal {
            status: 429,
            message: "Rate limit exceeded".into(),
        },
    );
    let mut e = engine(&parsed, &dir, &store, "changed", &mut hive);
    e.meter = Some(&meter);
    let summary = e.run().unwrap();
    assert_eq!(summary.reason, StopReason::MaxEpochs);
    assert_eq!(summary.warnings, Vec::<String>::new());
    assert!(
        !events(&store, "changed")
            .iter()
            .any(|k| k == "provider_refused")
    );
}

#[test]
fn an_empty_epoch_without_refusals_still_stalls_quietly() {
    let root = tempfile::tempdir().unwrap();
    let (dir, parsed) = fixture(root.path(), |_| {});
    let store = Store::new(root.path().join("home"));
    let (mut hive, meter) = provided(
        2,
        0,
        Box::new(|_, _| None),
        Refusal {
            status: 0,
            message: String::new(),
        },
    );
    let mut e = engine(&parsed, &dir, &store, "quiet", &mut hive);
    e.meter = Some(&meter);
    let summary = e.run().unwrap();
    assert_eq!(summary.reason, StopReason::Stalled);
    assert_eq!(summary.warnings, Vec::<String>::new());
    assert_eq!(summary.error, None);
    assert!(
        !events(&store, "quiet")
            .iter()
            .any(|k| k == "provider_refused")
    );
}
