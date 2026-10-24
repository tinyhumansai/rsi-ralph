//! Candidate failure evidence binds to a known design and survives replay.
use super::*;
use crate::{CandidateFailureEvent, CandidateFailureKind};

fn failure(candidate_id: &str, stage: &str, failed_runs: u32, runs: u32) -> CandidateFailureEvent {
    CandidateFailureEvent {
        candidate_id: candidate_id.into(),
        epoch: 4,
        stage: stage.into(),
        kind: CandidateFailureKind::Crash,
        failed_runs,
        runs,
    }
}

#[test]
fn candidate_failure_evidence_for_a_prepared_design_survives_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    let mut archive = Archive::open(&path).unwrap();
    let base = baseline(&mut archive);
    let (_, package) = child(&archive, &base, 30, Verdict::Rejected);
    let id = archive.prepare(package, BTreeMap::new()).unwrap();
    let event = failure(&id, "Screen", 2, 4);
    archive.record_candidate_failure(event.clone()).unwrap();
    assert_eq!(archive.candidate_failures(), vec![&event]);
    assert_eq!(
        serde_json::to_value(&event).unwrap(),
        serde_json::json!({
            "candidate_id": id, "epoch": 4, "stage": "Screen",
            "kind": "crash", "failed_runs": 2, "runs": 4,
        })
    );
    drop(archive);
    assert_eq!(
        Archive::open(&path).unwrap().candidate_failures(),
        vec![&event]
    );
}

#[test]
fn candidate_failure_evidence_rejects_unknown_designs_stages_and_counts() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("a.json")).unwrap();
    let base = baseline(&mut archive);
    let (_, package) = child(&archive, &base, 30, Verdict::Rejected);
    let unknown = format!("sha256:{}", "0".repeat(64));
    let id = archive.prepare(package, BTreeMap::new()).unwrap();
    for event in [
        failure(&unknown, "Screen", 1, 1),
        failure(&id, "Replay", 1, 1),
        failure(&id, "Screen", 0, 1),
        failure(&id, "Screen", 3, 2),
    ] {
        assert_eq!(
            archive.record_candidate_failure(event),
            Err(Error::InvalidRecord)
        );
    }
    assert_eq!(archive.candidate_failures().len(), 0);
    archive
        .record_candidate_failure(failure(&base, "Confirmation", 1, 1))
        .unwrap();
    assert_eq!(archive.candidate_failures().len(), 1);
}
