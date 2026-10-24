//! Snapshot isolation, learning preconditions, contamination and durable restore.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::*;
use tinyhivemind_lang::{MemoryIdentity, MemoryKind, MemoryLifecycle};

fn package() -> Package {
    let mut p = Package::default();
    p.constitution.permitted_edits = vec![crate::PatchClass::Memory];
    p.memory.root = "project:test".into();
    p.memory.identities.push(MemoryIdentity {
        id: "notes".into(),
        namespace: "project:test/agent:notes".into(),
        kind: MemoryKind::Agent,
        lifecycle: MemoryLifecycle::Hive,
        read_only: false,
        fork_parent: None,
    });
    p
}
fn learning(before: Option<&str>, after: Option<&str>) -> Patch {
    Patch {
        class: crate::PatchClass::Memory,
        operations: vec![Operation::Learning {
            identity: "notes".into(),
            entry: "method".into(),
            watermark: 1,
            before: before.map(str::to_owned),
            after: after.map(str::to_owned),
        }],
    }
}
#[test]
fn learning_is_pinned_without_changing_live_memory_and_restores_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.json");
    let store = SnapshotMemoryStore::open(&path, vec!["private-canary".into()], 100_000).unwrap();
    let p = package();
    let marks = BTreeMap::from([("notes".into(), 1)]);
    store
        .seed(
            &p,
            &marks,
            BTreeMap::from([(
                "notes".into(),
                BTreeMap::from([("method".into(), "old".into())]),
            )]),
        )
        .unwrap();
    store.activate(&p, &marks).unwrap();
    let state = CandidateState {
        package: p.clone(),
        memory_watermarks: marks.clone(),
    };
    let next = store
        .pin(&state, &learning(Some("old"), Some("new")))
        .unwrap();
    assert_eq!(next["notes"], 2);
    assert_eq!(store.active_watermarks().unwrap(), marks);
    assert_eq!(store.read(&p, &marks).unwrap()["notes"]["method"], "old");
    assert_eq!(store.read(&p, &next).unwrap()["notes"]["method"], "new");
    drop(store);
    let store = SnapshotMemoryStore::open(&path, vec!["private-canary".into()], 100_000).unwrap();
    store.activate(&p, &next).unwrap();
    store.activate(&p, &marks).unwrap();
    assert_eq!(store.active_watermarks().unwrap(), marks);
}
#[test]
fn refuses_stale_entries_readonly_targets_canaries_and_conflicting_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let store =
        SnapshotMemoryStore::open(dir.path().join("m.json"), vec!["secret".into()], 100_000)
            .unwrap();
    let mut p = package();
    let marks = BTreeMap::from([("notes".into(), 1)]);
    let values = BTreeMap::from([("notes".into(), BTreeMap::new())]);
    store.seed(&p, &marks, values.clone()).unwrap();
    let state = CandidateState {
        package: p.clone(),
        memory_watermarks: marks.clone(),
    };
    assert_eq!(
        store.pin(&state, &learning(Some("missing"), Some("new"))),
        Err(Error::InvalidRecord)
    );
    assert_eq!(
        store.pin(&state, &learning(None, Some("secret"))),
        Err(Error::InvalidRecord)
    );
    p.memory.identities[0].read_only = true;
    let state = CandidateState {
        package: p.clone(),
        memory_watermarks: marks.clone(),
    };
    assert_eq!(
        store.pin(&state, &learning(None, Some("new"))),
        Err(Error::InvalidRecord)
    );
    let changed = BTreeMap::from([(
        "notes".into(),
        BTreeMap::from([("x".into(), "different".into())]),
    )]);
    assert_eq!(store.seed(&p, &marks, changed), Err(Error::InvalidRecord));
    assert_eq!(store.read(&p, &BTreeMap::new()), Err(Error::InvalidRecord));
    assert_eq!(
        store.activate(&p, &BTreeMap::from([("notes".into(), 99)])),
        Err(Error::InvalidRecord)
    );
}
#[test]
fn separate_store_handles_preserve_data_and_forget_is_reversible() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.json");
    let a = SnapshotMemoryStore::open(&path, vec![], 100_000).unwrap();
    let b = SnapshotMemoryStore::open(&path, vec![], 100_000).unwrap();
    let p = package();
    let marks = BTreeMap::from([("notes".into(), 1)]);
    a.seed(
        &p,
        &marks,
        BTreeMap::from([(
            "notes".into(),
            BTreeMap::from([("method".into(), "old".into())]),
        )]),
    )
    .unwrap();
    let next = b
        .pin(
            &CandidateState {
                package: p.clone(),
                memory_watermarks: marks.clone(),
            },
            &learning(Some("old"), None),
        )
        .unwrap();
    assert!(a.read(&p, &next).unwrap()["notes"].is_empty());
    assert_eq!(b.read(&p, &marks).unwrap()["notes"]["method"], "old");
}
#[test]
fn rejects_invalid_limits_corrupt_state_and_oversized_learning() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.json");
    assert!(SnapshotMemoryStore::open(&path, vec![], 0).is_err());
    assert!(SnapshotMemoryStore::open(&path, vec![String::new()], 10000).is_err());
    std::fs::write(&path, "invalid").unwrap();
    assert!(matches!(
        SnapshotMemoryStore::open(&path, vec![], 10000),
        Err(Error::ArchiveCorrupt)
    ));
    std::fs::remove_file(&path).unwrap();
    let store = SnapshotMemoryStore::open(&path, vec![], 1000).unwrap();
    let p = package();
    let marks = BTreeMap::from([("notes".into(), 1)]);
    store
        .seed(
            &p,
            &marks,
            BTreeMap::from([("notes".into(), BTreeMap::new())]),
        )
        .unwrap();
    assert_eq!(
        store.pin(
            &CandidateState {
                package: p,
                memory_watermarks: marks
            },
            &learning(None, Some(&"x".repeat(2000)))
        ),
        Err(Error::InvalidRecord)
    );
}
#[test]
fn forks_copy_pinned_parent_and_retirement_keeps_parent_for_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let store = SnapshotMemoryStore::open(dir.path().join("m.json"), vec![], 100_000).unwrap();
    let mut p = package();
    p.constitution
        .permitted_edits
        .push(crate::PatchClass::Roster);
    let marks = BTreeMap::from([("notes".into(), 1)]);
    store
        .seed(
            &p,
            &marks,
            BTreeMap::from([(
                "notes".into(),
                BTreeMap::from([("method".into(), "original".into())]),
            )]),
        )
        .unwrap();
    let mut child = p.clone();
    child.memory.identities.push(MemoryIdentity {
        id: "child".into(),
        namespace: "project:test/agent:notes/agent:child".into(),
        fork_parent: Some("notes".into()),
        ..p.memory.identities[0].clone()
    });
    let patch = tinyhivemind_lang::patch::diff(&p, &child, crate::PatchClass::Memory).unwrap();
    let next = store
        .pin(
            &CandidateState {
                package: p.clone(),
                memory_watermarks: marks.clone(),
            },
            &patch,
        )
        .unwrap();
    assert_eq!(
        store.read(&child, &next).unwrap()["child"]["method"],
        "original"
    );
    let mut retired = child.clone();
    retired.memory.identities.remove(1);
    let patch =
        tinyhivemind_lang::patch::diff(&child, &retired, crate::PatchClass::Memory).unwrap();
    let retired_marks = store
        .pin(
            &CandidateState {
                package: child.clone(),
                memory_watermarks: next.clone(),
            },
            &patch,
        )
        .unwrap();
    assert_eq!(retired_marks, marks);
    store.activate(&retired, &retired_marks).unwrap();
    store.activate(&child, &next).unwrap();
    assert_eq!(
        store.read(&child, &next).unwrap()["child"]["method"],
        "original"
    );
}

#[test]
fn concurrent_readers_share_the_snapshot_lock() {
    let dir = tempfile::tempdir().unwrap();
    let store = SnapshotMemoryStore::open(dir.path().join("memory.json"), vec![], 100_000).unwrap();
    let other_reader = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&store.lock)
        .unwrap();
    FileExt::lock_shared(&other_reader).unwrap();
    assert_eq!(store.clone().active_watermarks().unwrap(), BTreeMap::new());
    assert_eq!(
        store.read(&Package::default(), &BTreeMap::new()).unwrap(),
        BTreeMap::new()
    );
    FileExt::unlock(&other_reader).unwrap();
}
