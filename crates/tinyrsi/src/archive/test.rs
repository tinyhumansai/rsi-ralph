//! Durable archive identity, activation, and transaction regressions.
#![expect(clippy::unwrap_used, reason = "assert test setup and outcomes")]
use super::*;
use tinyhivemind_lang::lineage::{Direction, ObjectiveScore};
use tinyhivemind_lang::patch::{Operation, Patch, PatchClass};

fn metadata(parent: Option<String>, patch: Patch, verdict: Verdict) -> VersionMetadata {
    VersionMetadata {
        parent,
        patch,
        rationale: "paired evaluation".into(),
        transcript_refs: vec![1],
        scores: BTreeMap::from([(
            "quality".into(),
            ObjectiveScore {
                value: 1.0,
                direction: Direction::Maximize,
            },
        )]),
        verdict,
        memory_watermarks: BTreeMap::new(),
    }
}
pub(super) fn baseline(archive: &mut Archive) -> String {
    let mut package = Package::default();
    package
        .constitution
        .permitted_edits
        .push(PatchClass::Policy);
    archive
        .record(
            package,
            metadata(
                None,
                Patch {
                    class: PatchClass::Policy,
                    operations: vec![],
                },
                Verdict::Accepted,
            ),
        )
        .unwrap()
}
fn child(archive: &Archive, parent: &str, turns: u64, verdict: Verdict) -> (Record, Package) {
    let original = archive.package(parent).unwrap();
    let old =
        serde_json::to_value(original).unwrap()["manifest"]["policies"]["episode"]["turn_budget"]
            .clone();
    let patch = Patch {
        class: PatchClass::Policy,
        operations: vec![Operation::Json {
            pointer: "/manifest/policies/episode/turn_budget".into(),
            before: Some(old),
            after: Some(serde_json::json!(turns)),
        }],
    };
    let package = tinyhivemind_lang::patch::apply(original, &patch)
        .unwrap()
        .candidate;
    let record =
        tinyhivemind_lang::lineage::record(&package, metadata(Some(parent.into()), patch, verdict))
            .unwrap();
    (record, package)
}
#[test]
fn persists_candidates_rejections_and_revert_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let base = baseline(&mut archive);
    let (accepted, package) = child(&archive, &base, 30, Verdict::Accepted);
    let next = accepted.id.clone();
    archive.append(accepted, package).unwrap();
    let (rejected, package) = child(&archive, &next, 31, Verdict::Rejected);
    archive.append(rejected, package).unwrap();
    assert_eq!(archive.head().unwrap().id, next);
    assert_eq!(archive.rejected().len(), 1);
    assert_eq!(archive.revert().unwrap(), base);
    drop(archive);
    let mut archive = Archive::open(path).unwrap();
    assert_eq!(archive.head().unwrap().id, base);
    assert_eq!(archive.records().len(), 3);
    assert_eq!(archive.revert(), Err(Error::NoParent));
}
#[test]
fn rejects_mismatched_duplicate_or_wrong_parent_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("archive.json")).unwrap();
    let base = baseline(&mut archive);
    let (record, package) = child(&archive, &base, 30, Verdict::Accepted);
    assert_eq!(
        archive.append(record.clone(), Package::default()),
        Err(Error::InvalidRecord)
    );
    archive.append(record.clone(), package.clone()).unwrap();
    assert_eq!(archive.append(record, package), Err(Error::InvalidRecord));
    let (record, package) = child(&archive, &base, 31, Verdict::Accepted);
    assert_eq!(archive.append(record, package), Err(Error::InvalidRecord));
    let (mut record, package) = child(&archive, &base, 32, Verdict::Rejected);
    record.parent = Some("missing".into());
    assert_eq!(archive.append(record, package), Err(Error::InvalidRecord));
    assert_eq!(archive.records().len(), 2);
}
#[test]
fn rejects_forged_patch_ancestry_even_for_matching_identity() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("archive.json")).unwrap();
    let base = baseline(&mut archive);
    let (mut record, package) = child(&archive, &base, 30, Verdict::Rejected);
    record.patch.operations.clear();
    assert_eq!(archive.append(record, package), Err(Error::InvalidRecord));
}
#[test]
fn refuses_truncated_or_tampered_history_and_releases_lock_on_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    baseline(&mut archive);
    drop(archive);
    let contents = fs::read(&path).unwrap();
    fs::write(&path, &contents[..contents.len() / 2]).unwrap();
    assert!(matches!(Archive::open(&path), Err(Error::ArchiveCorrupt)));
    let mut wire: serde_json::Value = serde_json::from_slice(&contents).unwrap();
    wire["events"][0]["record"]["id"] = serde_json::json!("forged");
    fs::write(&path, serde_json::to_vec(&wire).unwrap()).unwrap();
    assert!(matches!(Archive::open(&path), Err(Error::ArchiveCorrupt)));
    fs::write(&path, contents).unwrap();
    assert!(Archive::open(path).is_ok());
}
#[test]
fn locks_writer_until_drop() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let archive = Archive::open(&path).unwrap();
    assert!(matches!(Archive::open(&path), Err(Error::ArchiveLocked)));
    drop(archive);
    assert!(Archive::open(path).is_ok());
}
#[test]
fn failed_write_does_not_change_memory_or_persist_partial_record() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let base = baseline(&mut archive);
    let contents = fs::read(&path).unwrap();
    let (record, package) = child(&archive, &base, 30, Verdict::Accepted);
    fs::create_dir(archive.temporary_path()).unwrap();
    assert_eq!(archive.append(record, package), Err(Error::ArchiveIo));
    assert_eq!(archive.records().len(), 1);
    assert_eq!(fs::read(path).unwrap(), contents);
}
#[test]
fn retains_invalid_proposal_without_a_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let event = ProposalEvent {
        epoch: 4,
        patch: Patch {
            class: PatchClass::Prompt,
            operations: vec![],
        },
        rationale: "proposal".into(),
        reason: "invalid patch".into(),
        transcript_refs: vec![2],
    };
    archive.record_proposal(event.clone()).unwrap();
    drop(archive);
    let archive = Archive::open(path).unwrap();
    assert_eq!(archive.proposals(), vec![&event]);
    assert!(archive.head().is_none());
}

#[test]
fn rejects_replayed_duplicates_and_invalid_activation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let base = baseline(&mut archive);
    drop(archive);
    let original = fs::read(&path).unwrap();
    let mut snapshot: Snapshot = serde_json::from_slice(&original).unwrap();
    snapshot.events.push(snapshot.events[0].clone());
    fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    assert!(matches!(Archive::open(&path), Err(Error::ArchiveCorrupt)));
    let mut snapshot: Snapshot = serde_json::from_slice(&original).unwrap();
    snapshot.events.push(Event::Activation { id: base });
    fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    assert!(matches!(Archive::open(&path), Err(Error::ArchiveCorrupt)));
    let mut snapshot: Snapshot = serde_json::from_slice(&original).unwrap();
    snapshot.version = 3;
    fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    assert!(matches!(Archive::open(&path), Err(Error::ArchiveCorrupt)));
}

#[test]
fn baseline_requires_accepted_founder_without_parent_or_edits() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("archive.json")).unwrap();
    assert_eq!(archive.revert(), Err(Error::NoParent));
    let patch = Patch {
        class: PatchClass::Policy,
        operations: vec![],
    };
    let package = Package::default();
    assert_eq!(
        archive.record(
            package.clone(),
            metadata(None, patch.clone(), Verdict::Rejected)
        ),
        Err(Error::InvalidRecord)
    );
    assert_eq!(
        archive.record(
            package.clone(),
            metadata(Some("missing".into()), patch.clone(), Verdict::Accepted)
        ),
        Err(Error::InvalidRecord)
    );
    let mut invalid = package;
    invalid.manifest.id.clear();
    assert_eq!(
        archive.record(invalid, metadata(None, patch, Verdict::Accepted)),
        Err(Error::InvalidRecord)
    );
}

#[test]
fn learning_requires_parent_read_and_increased_result_watermarks() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("archive.json")).unwrap();
    let base = baseline(&mut archive);
    let mut parent = archive.records()[0].clone();
    parent.memory_watermarks.insert("shared".into(), 10);
    let mut record = parent.clone();
    record.patch = Patch {
        class: PatchClass::Memory,
        operations: vec![Operation::Learning {
            identity: "shared".into(),
            entry: "entry".into(),
            watermark: 10,
            before: None,
            after: Some("learned".into()),
        }],
    };
    assert_eq!(
        validate_watermarks(&parent, &record),
        Err(Error::InvalidRecord)
    );
    record.memory_watermarks.insert("shared".into(), 11);
    validate_watermarks(&parent, &record).unwrap();
    let Operation::Learning { watermark, .. } = &mut record.patch.operations[0] else {
        unreachable!()
    };
    *watermark = 9;
    assert_eq!(
        validate_watermarks(&parent, &record),
        Err(Error::InvalidRecord)
    );
    record.patch.operations.clear();
    assert_eq!(
        validate_watermarks(&parent, &record),
        Err(Error::InvalidRecord)
    );
    assert!(archive.package(&base).is_some());
}

#[test]
fn proposal_wire_pins_public_evidence_fields() {
    let proposal = ProposalEvent {
        epoch: 2,
        patch: Patch {
            class: PatchClass::Prompt,
            operations: vec![],
        },
        rationale: "grounded edit".into(),
        reason: "guarded".into(),
        transcript_refs: vec![9],
    };
    let wire = serde_json::json!({"epoch":2,"patch":{"class":"prompt","operations":[]},"rationale":"grounded edit","reason":"guarded","transcript_refs":[9]});
    assert_eq!(serde_json::to_value(&proposal).unwrap(), wire);
    assert_eq!(
        serde_json::from_value::<ProposalEvent>(wire).unwrap(),
        proposal
    );
}

#[test]
fn reports_io_errors_for_unusable_storage_paths() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file");
    fs::write(&file, b"occupied").unwrap();
    assert!(matches!(
        Archive::open(file.join("child")),
        Err(Error::ArchiveIo)
    ));
    let directory = dir.path().join("directory");
    fs::create_dir(&directory).unwrap();
    assert!(matches!(Archive::open(&directory), Err(Error::ArchiveIo)));
    assert!(!sidecar(&directory, "lock").exists());
}

#[cfg(unix)]
#[test]
fn refuses_symlink_archive_and_replaces_stale_pending_symlink_safely() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    fs::write(&target, b"preserve this file").unwrap();
    let path = dir.path().join("archive.json");
    symlink(&target, &path).unwrap();
    assert!(matches!(Archive::open(&path), Err(Error::ArchiveIo)));
    fs::remove_file(&path).unwrap();
    let mut archive = Archive::open(&path).unwrap();
    symlink(&target, archive.temporary_path()).unwrap();
    baseline(&mut archive);
    assert_eq!(fs::read(&target).unwrap(), b"preserve this file");
}

#[test]
fn preserves_unscored_prepared_candidate_across_host_failure_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let base = baseline(&mut archive);
    let (record, package) = child(&archive, &base, 30, Verdict::Accepted);
    assert_eq!(
        archive.prepare(package.clone(), BTreeMap::new()).unwrap(),
        record.id
    );
    assert_eq!(
        archive.prepare(package.clone(), BTreeMap::new()),
        Err(Error::InvalidRecord)
    );
    assert_eq!(archive.records().len(), 1);
    assert_eq!(archive.head().unwrap().id, base);
    drop(archive);
    let mut archive = Archive::open(path).unwrap();
    assert_eq!(archive.package(&record.id), Some(&package));
    archive.append(record, package).unwrap();
    assert_eq!(archive.records().len(), 2);
}

#[test]
fn prepared_replay_rejects_invalid_identity_and_watermarks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let package = Package::default();
    assert_eq!(
        archive.prepare(package.clone(), BTreeMap::from([("unknown".into(), 4)])),
        Err(Error::InvalidRecord)
    );
    archive.prepare(package, BTreeMap::new()).unwrap();
    drop(archive);
    let mut snapshot: Snapshot = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let Event::Prepared { id, .. } = &mut snapshot.events[0] else {
        unreachable!()
    };
    *id = "forged".into();
    fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    assert!(matches!(Archive::open(path), Err(Error::ArchiveCorrupt)));
}

#[test]
fn conclusion_persists_candidate_and_outcome_together_or_neither() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let base = baseline(&mut archive);
    let (record, package) = child(&archive, &base, 30, Verdict::Accepted);
    let metadata = metadata(record.parent.clone(), record.patch.clone(), record.verdict);
    let outcome = ProposalEvent {
        epoch: 5,
        patch: record.patch,
        rationale: "approved".into(),
        reason: "Accepted".into(),
        transcript_refs: vec![1],
    };
    let before = fs::read(&path).unwrap();
    fs::create_dir(archive.temporary_path()).unwrap();
    assert_eq!(
        archive.conclude(package.clone(), metadata.clone(), outcome.clone()),
        Err(Error::ArchiveIo)
    );
    assert_eq!(archive.records().len(), 1);
    assert_eq!(archive.proposals(), Vec::<&ProposalEvent>::new());
    assert_eq!(archive.head().unwrap().id, base);
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_dir(archive.temporary_path()).unwrap();
    let id = archive
        .conclude(package, metadata, outcome.clone())
        .unwrap();
    assert_eq!(id, record.id);
    drop(archive);
    let archive = Archive::open(path).unwrap();
    assert_eq!(archive.records().len(), 2);
    assert_eq!(archive.head().unwrap().id, id);
    assert_eq!(archive.proposals(), vec![&outcome]);
}

#[test]
fn rejects_learning_that_unfreezes_original_readonly_memory() {
    use tinyhivemind_lang::{MemoryIdentity, MemoryKind, MemoryLifecycle, patch::apply};
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("archive.json")).unwrap();
    let mut package = Package::default();
    package.constitution.permitted_edits = vec![PatchClass::Memory];
    package.memory.identities.push(MemoryIdentity {
        id: "shared".into(),
        namespace: "team:hive/agent:shared".into(),
        kind: MemoryKind::Pool,
        lifecycle: MemoryLifecycle::Hive,
        read_only: true,
        fork_parent: None,
    });
    let empty = Patch {
        class: PatchClass::Memory,
        operations: vec![],
    };
    let mut founding = metadata(None, empty, Verdict::Accepted);
    founding.memory_watermarks.insert("shared".into(), 10);
    let base = archive.record(package.clone(), founding).unwrap();
    let patch = Patch {
        class: PatchClass::Memory,
        operations: vec![
            Operation::Json {
                pointer: "/memory/identities/0/read_only".into(),
                before: Some(true.into()),
                after: Some(false.into()),
            },
            Operation::Learning {
                identity: "shared".into(),
                entry: "fact".into(),
                watermark: 10,
                before: None,
                after: Some("learned".into()),
            },
        ],
    };
    let candidate = apply(&package, &patch).unwrap().candidate;
    let mut evaluation = metadata(Some(base), patch, Verdict::Accepted);
    evaluation.memory_watermarks.insert("shared".into(), 11);
    assert_eq!(
        archive.record(candidate, evaluation),
        Err(Error::InvalidRecord)
    );
    assert_eq!(archive.records().len(), 1);
}

mod candidate_failure;
mod dispositions;
