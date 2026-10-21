//! A golden checker scored through the port, and its failure paths.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::{HostVerifier, VerifierDeclaration};

fn binding() -> VerificationBinding {
    VerificationBinding {
        episode_id: "ep".into(),
        seat_id: "seat".into(),
        node_id: "answer".into(),
        artifact_id: "a".into(),
        transcript_refs: vec![1],
    }
}

fn scorer(binding: VerificationBinding) -> VerifierScorer {
    let registry = VerifierRegistry::new(vec![VerifierDeclaration {
        node_id: "answer".into(),
        owner_seat: "seat".into(),
        verifier: HostVerifier::Golden {
            expected: b"42".to_vec(),
        },
    }])
    .unwrap();
    VerifierScorer::new(registry, binding)
}

#[test]
fn a_passing_artifact_scores_eligible_full_quality_on_every_seed() {
    let a = scorer(binding())
        .assess(b"42", &Goal::new("answer"), &[1, 2])
        .unwrap();
    assert_eq!(a.qualities, vec![1.0, 1.0]);
    assert_eq!(a.eligible, Some(vec![1.0, 1.0]));
    assert_eq!(
        a.rationale.as_deref(),
        Some("checker passed on node answer")
    );
}

#[test]
fn a_failing_artifact_scores_zero() {
    let a = scorer(binding())
        .assess(b"41", &Goal::new("answer"), &[1])
        .unwrap();
    assert_eq!(a.qualities, vec![0.0]);
    assert!(a.rationale.unwrap().contains("failed"));
}

#[test]
fn refuses_no_seeds_and_an_unauthorized_binding() {
    let goal = Goal::new("answer");
    assert_eq!(
        scorer(binding()).assess(b"42", &goal, &[]),
        Err(tinyrsi_score::Error::InvalidEvaluation)
    );
    let forged = VerificationBinding {
        seat_id: "intruder".into(),
        ..binding()
    };
    assert_eq!(
        scorer(forged).assess(b"42", &goal, &[1]),
        Err(tinyrsi_score::Error::InvalidEvaluation)
    );
}

#[test]
fn a_checker_that_cannot_run_is_a_host_failure() {
    let workspace = tempfile::tempdir().unwrap();
    let registry = VerifierRegistry::new(vec![VerifierDeclaration {
        node_id: "answer".into(),
        owner_seat: "seat".into(),
        verifier: HostVerifier::Process(crate::CommandSpec {
            program: "/etc/passwd".into(),
            args: vec![],
            workspace_root: workspace.path().to_path_buf(),
            timeout_ms: 1000,
            max_output_bytes: 64,
        }),
    }])
    .unwrap();
    assert_eq!(
        VerifierScorer::new(registry, binding()).assess(b"x", &Goal::new("g"), &[1]),
        Err(tinyrsi_score::Error::HostFailed)
    );
}

fn lean_scorer(binding: VerificationBinding) -> VerifierScorer {
    let workspace = tempfile::tempdir().unwrap();
    let registry = VerifierRegistry::new(vec![VerifierDeclaration {
        node_id: "answer".into(),
        owner_seat: "seat".into(),
        verifier: HostVerifier::Lean {
            command: crate::CommandSpec {
                program: "/usr/bin/python3".into(),
                args: vec!["{artifact}".into()],
                workspace_root: workspace.path().to_path_buf(),
                timeout_ms: 1000,
                max_output_bytes: 64,
            },
            theorem: "theorem rsi_goal : True :=".into(),
        },
    }])
    .unwrap();
    VerifierScorer::new(registry, binding)
}

#[test]
fn a_proof_that_is_not_utf8_scores_zero() {
    let a = lean_scorer(binding())
        .assess(&[0xff, 0xfe], &Goal::new("answer"), &[1, 2])
        .unwrap();
    assert_eq!(a.qualities, vec![0.0, 0.0]);
    assert_eq!(
        a.rationale.as_deref(),
        Some("checker failed on node answer")
    );
}

#[test]
fn a_proof_that_is_not_utf8_still_refuses_an_unauthorized_binding() {
    let forged = VerificationBinding {
        seat_id: "intruder".into(),
        ..binding()
    };
    assert_eq!(
        lean_scorer(forged).assess(&[0xff, 0xfe], &Goal::new("answer"), &[1]),
        Err(tinyrsi_score::Error::InvalidEvaluation)
    );
}
