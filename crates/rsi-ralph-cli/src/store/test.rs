//! Run folders, listing, interruption and atomic JSON.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;

fn record(id: &str, started_at: u64, status: RunStatus, pid: u32) -> RunRecord {
    RunRecord {
        id: id.into(),
        r#loop: "demo".into(),
        status,
        pid,
        started_at,
        finished_at: None,
        stop_reason: None,
        incumbent: "incumbent".into(),
        incumbent_score: Some(7.0),
        epochs: 1,
        spend_usd: 0.25,
        error: None,
        warnings: Vec::new(),
    }
}

#[test]
fn creates_a_run_with_a_frozen_loop_copy() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::new(home.path());
    let files = BTreeMap::from([
        ("rsi.json".to_owned(), "{}".to_owned()),
        ("hive/seats/a.md".to_owned(), "a".to_owned()),
    ]);
    let run = store.create_run("demo", Some("r1"), &files).unwrap();
    assert_eq!(run.path, home.path().join("runs/demo/r1"));
    assert_eq!(
        fs::read_to_string(run.loop_dir().join("hive/seats/a.md")).unwrap(),
        "a"
    );
    assert!(run.epoch_dir(2).unwrap().ends_with("epoch-2"));
    assert!(
        matches!(store.create_run("demo", Some("r1"), &files), Err(Error::RunExists(id)) if id == "r1")
    );
    assert!(matches!(
        store.create_run("demo", Some("../x"), &files),
        Err(Error::Path { .. })
    ));
    let generated = store.create_run("demo", None, &BTreeMap::new()).unwrap();
    assert_eq!(generated.id.len(), 15);
}

#[test]
fn lists_runs_newest_first_and_marks_dead_runs_interrupted() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::new(home.path());
    let a = store
        .create_run("demo", Some("a"), &BTreeMap::new())
        .unwrap();
    write_json(&a.record_path(), &record("a", 10, RunStatus::Finished, 1)).unwrap();
    let b = store
        .create_run("demo", Some("b"), &BTreeMap::new())
        .unwrap();
    // A pid far above any real process stands in for a dead owner.
    write_json(
        &b.record_path(),
        &record("b", 20, RunStatus::Running, 4_000_000),
    )
    .unwrap();
    let c = store
        .create_run("other", Some("c"), &BTreeMap::new())
        .unwrap();
    write_json(
        &c.record_path(),
        &record("c", 30, RunStatus::Running, std::process::id()),
    )
    .unwrap();
    fs::write(store.runs_root().join("demo/a/../junk.json"), "x").unwrap();

    let all = store.list_runs(None).unwrap();
    assert_eq!(
        all.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["c", "b", "a"]
    );
    assert_eq!(all[0].status, RunStatus::Running);
    assert_eq!(all[1].status, RunStatus::Interrupted);
    assert_eq!(store.list_runs(Some("demo")).unwrap().len(), 2);
    let (dir, found) = store.find_run("demo/a").unwrap();
    assert_eq!((dir.id.as_str(), found.status), ("a", RunStatus::Finished));
    assert!(matches!(store.find_run("demo/zzz"), Err(Error::NoRun(_))));
    assert!(matches!(store.find_run("nodash"), Err(Error::NoRun(_))));
    assert_eq!(
        Store::new(home.path().join("empty"))
            .list_runs(None)
            .unwrap(),
        vec![]
    );
}

#[test]
fn pins_the_run_record_stored_form() {
    let json = serde_json::to_value(record("a", 10, RunStatus::Finished, 1)).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "id": "a", "loop": "demo", "status": "finished", "pid": 1, "started_at": 10,
            "finished_at": null, "stop_reason": null, "incumbent": "incumbent",
            "incumbent_score": 7.0, "epochs": 1, "spend_usd": 0.25, "error": null
        })
    );
}

#[test]
fn run_record_warnings_are_omitted_when_empty_and_default_when_missing() {
    let mut warned = record("a", 10, RunStatus::Finished, 1);
    warned.stop_reason = Some(rsi_ralph_trace::StopReason::ProviderRefused);
    warned.warnings = vec!["epoch 1: provider refused 1 of 2 model calls (429: slow)".into()];
    let json = serde_json::to_value(&warned).unwrap();
    assert_eq!(json["stop_reason"], "provider_refused");
    assert_eq!(
        json["warnings"],
        serde_json::json!(["epoch 1: provider refused 1 of 2 model calls (429: slow)"])
    );
    assert_eq!(serde_json::from_value::<RunRecord>(json).unwrap(), warned);
    // A `run.json` written before warnings existed still reads.
    let mut old = serde_json::to_value(record("a", 10, RunStatus::Finished, 1)).unwrap();
    old.as_object_mut().unwrap().remove("warnings");
    assert_eq!(
        serde_json::from_value::<RunRecord>(old).unwrap().warnings,
        Vec::<String>::new()
    );
}

#[test]
fn json_documents_round_trip_and_report_decode_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep/x.json");
    write_json(&path, &vec![1, 2]).unwrap();
    assert_eq!(read_json::<Vec<u32>>(&path).unwrap(), vec![1, 2]);
    assert!(matches!(
        read_json::<String>(&path),
        Err(Error::Store { .. })
    ));
    assert!(matches!(
        read_json::<String>(&dir.path().join("none")),
        Err(Error::Io { .. })
    ));
}

#[test]
fn timestamps_are_utc_calendar_times() {
    assert_eq!(timestamp_id(0), "19700101-000000");
    assert_eq!(timestamp_id(1_791_763_200 + 3_723), "20261012-010203");
    assert_eq!(timestamp_id(951_782_400), "20000229-000000");
}

#[test]
fn default_home_prefers_tinyrsi_home() {
    let both = |k: &str| match k {
        "TINYRSI_HOME" => Some("/t".to_owned()),
        "HOME" => Some("/h".to_owned()),
        _ => None,
    };
    assert_eq!(Store::default_home(&both), PathBuf::from("/t"));
    let home = |k: &str| (k == "HOME").then(|| "/h".to_owned());
    assert_eq!(Store::default_home(&home), PathBuf::from("/h/.tinyrsi"));
    assert_eq!(Store::default_home(&|_| None), PathBuf::from(".tinyrsi"));
    let store = Store::new("/s");
    assert_eq!(store.home(), Path::new("/s"));
    assert_eq!(store.spend_ledger(), PathBuf::from("/s/spend.json"));
    assert_eq!(store.cache_dir("l"), PathBuf::from("/s/cache/l"));
    assert_eq!(store.retreat_dir("l"), PathBuf::from("/s/retreat/l"));
}

#[test]
fn scrub_replaces_secret_values() {
    assert_eq!(scrub("key sk-abcd in text", ["sk-abcd"]), "key *** in text");
    assert_eq!(scrub("ab", ["ab"]), "ab");
}
