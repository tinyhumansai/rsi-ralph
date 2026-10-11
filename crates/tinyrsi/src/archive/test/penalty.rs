//! The strike ledger: shapes, strike sources, exclusions, windows and revocation.
use super::*;
use crate::{PatchShape, PenaltyPolicy, PenaltyReport, Strike, StrikeKind};

fn context(item: &str, after: &str) -> Patch {
    Patch {
        class: PatchClass::Context,
        operations: vec![Operation::Context {
            path: "context/main.md".into(),
            item: item.into(),
            before: None,
            after: Some(after.into()),
        }],
    }
}
fn stop(archive: &mut Archive, epoch: u64, patch: &Patch, reason: &str) {
    archive
        .record_proposal(ProposalEvent {
            epoch,
            patch: patch.clone(),
            rationale: "r".into(),
            reason: reason.into(),
            transcript_refs: vec![1],
        })
        .unwrap();
}
fn strike(epoch: u64, kind: StrikeKind) -> Strike {
    Strike {
        shape: PatchShape::of(&context("tools", "a")),
        epoch,
        kind,
        proposal: None,
    }
}

#[test]
fn patch_shapes_ignore_literal_values_but_not_targets() {
    let shape = PatchShape::of(&context("tools", "Use the calculator."));
    assert_eq!(
        shape,
        PatchShape::of(&context("tools", "Call a calculator."))
    );
    assert_ne!(
        shape,
        PatchShape::of(&context("method", "Use the calculator."))
    );
    assert_eq!(shape.class, PatchClass::Context);
    assert_eq!(
        shape.targets,
        vec![("context".to_owned(), "context/main.md#tools".to_owned())]
    );
    assert_eq!(shape.to_string(), "context: context context/main.md#tools");
    let mixed = Patch {
        class: PatchClass::Memory,
        operations: vec![
            Operation::Learning {
                identity: "notes".into(),
                entry: "k".into(),
                watermark: 3,
                before: None,
                after: Some("v".into()),
            },
            Operation::Json {
                pointer: "/memory/bindings".into(),
                before: None,
                after: None,
            },
            Operation::Json {
                pointer: "/memory/bindings".into(),
                before: Some(serde_json::json!(1)),
                after: None,
            },
        ],
    };
    assert_eq!(
        PatchShape::of(&mixed).targets,
        vec![
            ("json".to_owned(), "/memory/bindings".to_owned()),
            ("learning".to_owned(), "notes#k".to_owned()),
        ]
    );
    let documents = Patch {
        class: PatchClass::Prompt,
        operations: vec![
            Operation::Prompt {
                path: "seats/a.md".into(),
                before: "x".into(),
                after: "y".into(),
            },
            Operation::Document {
                path: "roles/b.md".into(),
                before: None,
                after: Some("z".into()),
            },
        ],
    };
    assert_eq!(
        PatchShape::of(&documents).targets,
        vec![
            ("document".to_owned(), "roles/b.md".to_owned()),
            ("prompt".to_owned(), "seats/a.md".to_owned()),
        ]
    );
}

#[test]
fn only_harmful_stops_strike_among_proposal_reasons() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("a.json")).unwrap();
    baseline(&mut archive);
    let patch = context("tools", "a");
    for reason in [
        "HostFailure",
        "Inconclusive",
        "ApprovalDenied",
        "Guard",
        "Untestable",
        "Duplicate",
        "Cooldown",
        // No normalized evidence: a flat, not a confident regression.
        "NoGain",
        "NOT_REPLICATED",
    ] {
        stop(&mut archive, 2, &patch, reason);
    }
    assert_eq!(archive.penalties().strikes.len(), 0);
    stop(&mut archive, 2, &patch, "Harmful");
    stop(&mut archive, 3, &patch, "HARMFUL");
    let report = archive.penalties();
    assert_eq!(
        report.strikes,
        vec![
            Strike {
                proposal: Some(9),
                ..strike(2, StrikeKind::Harmful)
            },
            Strike {
                proposal: Some(10),
                ..strike(3, StrikeKind::Harmful)
            },
        ]
    );
}

#[test]
fn a_strike_ages_out_of_the_window() {
    let policy = PenaltyPolicy::default();
    assert_eq!((policy.strikes, policy.window_epochs), (2, 3));
    let shape = PatchShape::of(&context("tools", "b"));
    let harmful = PenaltyReport {
        strikes: vec![strike(4, StrikeKind::Harmful)],
    };
    for (epoch, cooled) in [(3, false), (4, true), (6, true), (7, false)] {
        assert_eq!(harmful.cooled(&shape, epoch, &policy), cooled, "{epoch}");
    }
    assert!(!harmful.cooled(&PatchShape::of(&context("method", "b")), 4, &policy));
    let revoked = PenaltyReport {
        strikes: vec![strike(4, StrikeKind::Revoked)],
    };
    assert!(revoked.cooled(&shape, 5, &policy));
    let regressed = PenaltyReport {
        strikes: vec![
            strike(1, StrikeKind::Regressed),
            strike(3, StrikeKind::Regressed),
        ],
    };
    assert!(!regressed.cooled(&shape, 2, &policy));
    assert!(regressed.cooled(&shape, 3, &policy));
    assert!(!regressed.cooled(&shape, 4, &policy));
    for disabled in [
        PenaltyPolicy {
            strikes: 0,
            window_epochs: 3,
        },
        PenaltyPolicy {
            strikes: 2,
            window_epochs: 0,
        },
    ] {
        assert!(!harmful.cooled(&shape, 4, &disabled));
    }
    assert_eq!(
        serde_json::to_value(policy).unwrap(),
        serde_json::json!({"strikes": 2, "window_epochs": 3})
    );
}

#[test]
fn a_revocation_is_a_strike_and_keeps_the_record_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    let mut archive = Archive::open(&path).unwrap();
    let base = baseline(&mut archive);
    let (accepted, package) = child(&archive, &base, 30, Verdict::Accepted);
    let (id, revoked) = (accepted.id.clone(), accepted.patch.clone());
    archive.append(accepted, package).unwrap();
    assert_eq!(archive.revoke(5).unwrap(), base);
    assert_eq!(archive.head().unwrap().id, base);
    let check = |archive: &Archive| {
        let record = archive.records().iter().find(|r| r.id == id).unwrap();
        assert_eq!(record.verdict, Verdict::Accepted);
        assert_eq!(
            archive.penalties().strikes,
            vec![Strike {
                shape: PatchShape::of(&revoked),
                epoch: 5,
                kind: StrikeKind::Revoked,
                proposal: None,
            }]
        );
    };
    check(&archive);
    assert_eq!(archive.revoke(6), Err(Error::NoParent));
    drop(archive);
    check(&Archive::open(&path).unwrap());
}
