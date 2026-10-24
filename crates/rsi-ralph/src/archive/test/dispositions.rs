//! Safe duplicate history uses earlier durable events without leaking arbitrary text.
use super::*;
use crate::{DuplicateDisposition, ReasonCode};

fn stop(archive: &mut Archive, patch: &Patch, reason: &str) {
    archive
        .record_proposal(ProposalEvent {
            epoch: 7,
            patch: patch.clone(),
            rationale: "private-rationale-canary".into(),
            reason: reason.into(),
            transcript_refs: vec![1],
        })
        .unwrap();
}

#[test]
fn unresolved_duplicate_chains_do_not_change_after_later_acceptance_or_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let archive_path = dir.path().join("a.json");
    let mut archive = Archive::open(&archive_path).unwrap();
    let base = baseline(&mut archive);
    let (accepted, package) = child(&archive, &base, 30, Verdict::Accepted);
    let patch = accepted.patch.clone();
    archive.prepare(package.clone(), BTreeMap::new()).unwrap();
    stop(&mut archive, &patch, "Duplicate");
    stop(&mut archive, &patch, "HostFailure");
    stop(&mut archive, &patch, "DUPLICATE");
    let prefix = archive.proposal_dispositions();
    assert_eq!(
        prefix.rows[0].duplicate,
        Some(DuplicateDisposition::Unresolved)
    );
    assert_eq!(
        prefix.rows[2].duplicate,
        Some(DuplicateDisposition::Unresolved)
    );
    archive.append(accepted, package).unwrap();
    stop(&mut archive, &patch, "Duplicate");
    assert_eq!(
        archive.proposal_dispositions().rows[3].duplicate,
        Some(DuplicateDisposition::Accepted)
    );
    archive.revert().unwrap();
    stop(&mut archive, &patch, "Duplicate");
    let report = archive.proposal_dispositions();
    assert_eq!(&report.rows[..3], prefix.rows.as_slice());
    assert_eq!(
        report.rows[4].duplicate,
        Some(DuplicateDisposition::Accepted)
    );
    drop(archive);
    assert_eq!(
        Archive::open(archive_path).unwrap().proposal_dispositions(),
        report
    );
}

#[test]
fn accepted_claims_and_unknown_legacy_reasons_do_not_manufacture_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("a.json")).unwrap();
    let base = baseline(&mut archive);
    let (record, _) = child(&archive, &base, 31, Verdict::Rejected);
    stop(&mut archive, &record.patch, "private-reason-canary");
    stop(&mut archive, &record.patch, "Accepted");
    stop(&mut archive, &record.patch, "Duplicate");
    let report = archive.proposal_dispositions();
    assert_eq!(report.rows[0].reason, None);
    assert_eq!(report.rows[1].reason, Some(ReasonCode::Accepted));
    assert_eq!(
        report.rows[2].duplicate,
        Some(DuplicateDisposition::Unresolved)
    );
    let wire = serde_json::to_value(&report).unwrap();
    assert_eq!(
        wire,
        serde_json::json!({"version":1,"rows":[
            {"event_index":1,"epoch":7,"reason":null,"duplicate":null},
            {"event_index":2,"epoch":7,"reason":"ACCEPTED","duplicate":null},
            {"event_index":3,"epoch":7,"reason":"DUPLICATE","duplicate":"DUP_UNRESOLVED"}
        ]})
    );
    let text = wire.to_string();
    assert!(!text.contains("private-reason-canary"));
    assert!(!text.contains("private-rationale-canary"));
    assert!(!text.contains("turn_budget"));
    assert_eq!(
        serde_json::from_value::<crate::ProposalDispositionReport>(wire).unwrap(),
        report
    );
}

#[test]
fn explicit_terminal_rejections_and_rejected_records_supply_rejected_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("a.json")).unwrap();
    let base = baseline(&mut archive);
    let (rejected, package) = child(&archive, &base, 30, Verdict::Rejected);
    let patch = rejected.patch.clone();
    archive.append(rejected, package).unwrap();
    stop(&mut archive, &patch, "Duplicate");
    let (unrecorded, _) = child(&archive, &base, 31, Verdict::Rejected);
    stop(&mut archive, &unrecorded.patch, "Guard");
    stop(&mut archive, &unrecorded.patch, "Duplicate");
    stop(&mut archive, &unrecorded.patch, "Duplicate");
    let report = archive.proposal_dispositions();
    for index in [0, 2, 3] {
        assert_eq!(
            report.rows[index].duplicate,
            Some(DuplicateDisposition::Rejected)
        );
    }
    assert_eq!(report.rows[1].duplicate, None);
}

#[test]
fn both_coarse_reason_spellings_are_recognized_without_changing_wire_contract() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("a.json")).unwrap();
    let base = baseline(&mut archive);
    let (record, _) = child(&archive, &base, 31, Verdict::Rejected);
    let reasons = [
        ReasonCode::Untestable,
        ReasonCode::Ungrounded,
        ReasonCode::WrongClass,
        ReasonCode::Guard,
        ReasonCode::Unformalizable,
        ReasonCode::Duplicate,
        ReasonCode::NoGain,
        ReasonCode::Inconclusive,
        ReasonCode::NotReplicated,
        ReasonCode::ApprovalDenied,
        ReasonCode::HostFailure,
        ReasonCode::Accepted,
        ReasonCode::Harmful,
        ReasonCode::Cooldown,
    ];
    for reason in reasons {
        stop(&mut archive, &record.patch, &format!("{reason:?}"));
        let wire = serde_json::to_value(reason).unwrap();
        stop(&mut archive, &record.patch, wire.as_str().unwrap());
    }
    let report = archive.proposal_dispositions();
    assert_eq!(report.rows.len(), 28);
    for (rows, expected) in report.rows.chunks(2).zip(reasons) {
        assert_eq!(rows[0].reason, Some(expected));
        assert_eq!(rows[1].reason, Some(expected));
    }
    assert_eq!(
        serde_json::to_value(DuplicateDisposition::Accepted).unwrap(),
        "DUP_ACCEPTED"
    );
    assert_eq!(
        serde_json::to_value(DuplicateDisposition::Rejected).unwrap(),
        "DUP_REJECTED"
    );
}

#[test]
fn a_cooled_stop_is_not_rejected_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("a.json")).unwrap();
    let base = baseline(&mut archive);
    let (record, _) = child(&archive, &base, 31, Verdict::Rejected);
    stop(&mut archive, &record.patch, "Cooldown");
    stop(&mut archive, &record.patch, "Duplicate");
    let report = archive.proposal_dispositions();
    assert_eq!(report.rows[0].reason, Some(ReasonCode::Cooldown));
    assert_eq!(
        report.rows[1].duplicate,
        Some(DuplicateDisposition::Unresolved)
    );
}
