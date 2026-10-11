//! Regression tests for frozen assignments, matched trials, and counted failures.

#![expect(clippy::unwrap_used, reason = "assert deterministic fixture outcomes")]

use super::*;
use crate::{FamilyAnchor, ScorePenalties};

fn spec() -> ExperimentProtocolSpec {
    ExperimentProtocolSpec {
        original_config_id: "original".into(),
        evolved_config_id: "evolved".into(),
        initial_hive_id: "solver".into(),
        suites: SuiteFingerprints {
            selection: "selection".into(),
            confirmation: "confirmation".into(),
            anchor: "anchor".into(),
        },
        trials: (0..4)
            .map(|n| TrialSeeds {
                proposal_seed: n,
                evaluation_seeds: vec![100 + n * 2, 101 + n * 2],
            })
            .collect(),
        attempts_per_trial: 1,
        generation_cap: 100,
        evaluation_cap: 100,
        per_run_budget: crate::Budget { max_tokens: 10 },
        scoring: crate::ScoringPolicySpec {
            anchors: vec![FamilyAnchor {
                family: "quality".into(),
                baseline: 0.,
                reference: 1.,
                weight: 1.,
                regression_limit: 0.,
            }],
            penalties: ScorePenalties {
                false_claims: 1.,
                tamper: 1.,
                cost: 1.,
            },
            margin: 0.01,
            cost_tolerance: 0.,
            resamples: 200,
            bootstrap_seed: 42,
        },
        efficiency_margin: 0.01,
        source: EvidenceSource::Live,
    }
}

#[test]
fn freezes_protocol_before_any_execution() {
    let protocol = ExperimentProtocol::new(spec()).unwrap();
    assert_eq!(protocol.fingerprint().len(), 64);
    let rebuilt = ExperimentProtocol::new(protocol.spec().clone()).unwrap();
    assert_eq!(protocol.fingerprint(), rebuilt.fingerprint());
    let mut changed = protocol.spec().clone();
    changed.generation_cap += 1;
    assert_ne!(
        protocol.fingerprint(),
        ExperimentProtocol::new(changed).unwrap().fingerprint()
    );
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Good,
    AllFail,
    OriginalFail,
    EvolvedFail,
    WrongRequest,
    MissingAttempt,
    WrongSeeds,
    WrongTasks,
    DifferentInitial,
    GenerationOverCap,
    EvaluationOverCap,
    InvalidHealth,
    Negative,
    Variable,
    ZeroUsage,
    HardGate,
    NoAccepted,
    MixedAttempts,
}

#[derive(Debug)]
struct Host {
    mode: Mode,
    requests: Vec<TrialRequest>,
}

impl ExperimentExecutor for Host {
    fn execute_trial(&mut self, request: &TrialRequest) -> crate::Result<TrialEvidence> {
        self.requests.push(request.clone());
        if matches!(self.mode, Mode::AllFail)
            || matches!(self.mode, Mode::OriginalFail) && request.arm == ExperimentArm::Original
            || matches!(self.mode, Mode::EvolvedFail) && request.arm == ExperimentArm::Evolved
        {
            return Err(crate::Error::HostFailed);
        }
        let quality = match (self.mode, request.arm) {
            (Mode::Negative, _) => 0.2,
            (Mode::Variable, ExperimentArm::Evolved) => {
                if request.seeds.proposal_seed.is_multiple_of(2) {
                    0.9
                } else {
                    0.1
                }
            }
            (_, ExperimentArm::Original) => 0.4,
            (_, ExperimentArm::Evolved) => 0.8,
        };
        let runs = |quality| {
            request
                .seeds
                .evaluation_seeds
                .iter()
                .map(|seed| crate::EvalRun {
                    task: "private-task-content-must-not-leak".into(),
                    family: "quality".into(),
                    seed: *seed,
                    quality,
                    tokens: 1,
                    false_claims: 0,
                    tamper: 0,
                })
                .collect::<Vec<_>>()
        };
        let health = || crate::ScoreHealth {
            false_claim_rate: 0.,
            tamper_rate: 0.,
            cost_ratio: 1.,
        };
        let mut evidence = TrialEvidence {
            request: request.clone(),
            attempts: (0..request.attempts_per_trial)
                .map(|_| AttemptEvidence {
                    status: AttemptStatus::Accepted,
                    generation_tokens: 10,
                    evaluation_tokens: 0,
                })
                .collect(),
            initial_anchor: runs(0.2),
            final_anchor: runs(quality),
            initial_health: health(),
            final_health: health(),
        };
        distort(self.mode, request, &mut evidence);
        Ok(evidence)
    }
}

fn distort(mode: Mode, request: &TrialRequest, evidence: &mut TrialEvidence) {
    match mode {
        Mode::WrongRequest => evidence.request.config_id = "unregistered-config".into(),
        Mode::MissingAttempt => {
            evidence.attempts.pop();
        }
        Mode::WrongSeeds => evidence.initial_anchor[0].seed = 9999,
        Mode::WrongTasks if request.seeds.proposal_seed != 0 => {
            for run in evidence
                .initial_anchor
                .iter_mut()
                .chain(&mut evidence.final_anchor)
            {
                run.task = "different-task".into();
            }
        }
        Mode::DifferentInitial if request.arm == ExperimentArm::Evolved => {
            for run in &mut evidence.initial_anchor {
                run.quality = 0.3;
            }
        }
        Mode::GenerationOverCap => {
            evidence.attempts[0].generation_tokens = request.generation_cap + 1;
        }
        Mode::EvaluationOverCap => {
            evidence.attempts[0].evaluation_tokens = request.evaluation_cap;
        }
        Mode::InvalidHealth => evidence.final_health.cost_ratio = 0.,
        Mode::HardGate => evidence.final_health.tamper_rate = 0.1,
        Mode::NoAccepted => evidence.attempts[0].status = AttemptStatus::Rejected,
        Mode::MixedAttempts => {
            for (index, attempt) in evidence.attempts.iter_mut().enumerate() {
                attempt.status = match index % 4 {
                    0 => AttemptStatus::Accepted,
                    1 => AttemptStatus::Rejected,
                    2 => AttemptStatus::Inconclusive,
                    _ => AttemptStatus::Failed,
                };
            }
        }
        Mode::ZeroUsage => {
            for run in evidence
                .initial_anchor
                .iter_mut()
                .chain(&mut evidence.final_anchor)
            {
                run.tokens = 0;
            }
            evidence.attempts[0].generation_tokens = 0;
        }
        _ => {}
    }
}

fn execute(mode: Mode, source: EvidenceSource) -> (ExperimentReport, Host) {
    let mut specification = spec();
    specification.source = source;
    let protocol = ExperimentProtocol::new(specification).unwrap();
    let mut host = Host {
        mode,
        requests: Vec::new(),
    };
    (run_experiment(&protocol, &mut host).unwrap(), host)
}

#[test]
fn runs_twenty_four_matched_attempts_and_counts_both_anchor_matrices() {
    let (report, host) = execute(Mode::Good, EvidenceSource::Live);
    assert_eq!(report.attempts, 24);
    assert_eq!(report.generation_tokens, 240);
    assert_eq!(report.evaluation_tokens, 96);
    assert_eq!(report.verdict, ExperimentVerdict::SupportedImprovement);
    assert_eq!(host.requests.len(), 24);
    assert_eq!(host.requests[0].arm, ExperimentArm::Original);
    assert_eq!(host.requests[2].arm, ExperimentArm::Evolved);
    assert!(report.trials.iter().all(|t| t.verified && t.accepted == 1));
    for ablation in &report.ablations {
        assert_eq!(ablation.pairs, 4);
        assert!((ablation.efficiency_delta - 0.4 * 1000. / 14.).abs() < 1e-12);
        assert_eq!(ablation.lower, ablation.upper);
    }
    let public = serde_json::to_string(&report).unwrap();
    assert!(!public.contains("private-task-content"));
    assert!(!public.contains("per_family"));
    assert_eq!(
        serde_json::from_str::<ExperimentReport>(&public).unwrap(),
        report
    );
}

#[test]
fn scripted_gains_never_establish_model_improvement() {
    assert_eq!(
        execute(Mode::Good, EvidenceSource::Scripted).0.verdict,
        ExperimentVerdict::ProtocolOnly
    );
}

#[test]
fn negative_and_variable_results_remain_honest() {
    assert_eq!(
        execute(Mode::Negative, EvidenceSource::Live).0.verdict,
        ExperimentVerdict::Negative
    );
    let report = execute(Mode::Variable, EvidenceSource::Live).0;
    assert_eq!(report.verdict, ExperimentVerdict::Inconclusive);
    assert!(report.ablations[0].lower < report.ablations[0].upper);
    assert_eq!(report, execute(Mode::Variable, EvidenceSource::Live).0);
}

#[test]
fn errors_charge_both_reservations_and_never_drop_failed_attempts() {
    let report = execute(Mode::AllFail, EvidenceSource::Live).0;
    assert_eq!(report.attempts, 24);
    assert_eq!(report.generation_tokens, 2400);
    assert_eq!(report.evaluation_tokens, 2400);
    assert_eq!(report.verdict, ExperimentVerdict::Inconclusive);
    assert!(
        report
            .trials
            .iter()
            .all(|t| t.failed == 1 && t.failure == Some(TrialFailure::HostError))
    );
    assert!(report.ablations.iter().all(|a| a.pairs == 0));
    for mode in [Mode::OriginalFail, Mode::EvolvedFail] {
        let report = execute(mode, EvidenceSource::Live).0;
        assert_eq!(report.attempts, 24);
        assert_eq!(report.trials.iter().filter(|t| t.verified).count(), 12);
        assert_eq!(report.verdict, ExperimentVerdict::Inconclusive);
    }
}

#[test]
fn rejects_unregistered_assignments_draws_shapes_costs_and_caps() {
    for mode in [
        Mode::WrongRequest,
        Mode::MissingAttempt,
        Mode::WrongSeeds,
        Mode::WrongTasks,
        Mode::DifferentInitial,
        Mode::GenerationOverCap,
        Mode::EvaluationOverCap,
        Mode::InvalidHealth,
        Mode::ZeroUsage,
    ] {
        let report = execute(mode, EvidenceSource::Live).0;
        assert_eq!(report.verdict, ExperimentVerdict::Inconclusive, "{mode:?}");
        assert_eq!(report.attempts, 24, "{mode:?}");
        assert!(
            report
                .trials
                .iter()
                .any(|t| t.failure == Some(TrialFailure::InvalidEvidence)),
            "{mode:?}"
        );
    }
}

#[test]
fn rejects_invalid_preregistration_before_callbacks() {
    let changes: Vec<fn(&mut ExperimentProtocolSpec)> = vec![
        |s| s.original_config_id.clear(),
        |s| s.evolved_config_id = s.original_config_id.clone(),
        |s| s.suites.anchor = s.suites.selection.clone(),
        |s| s.trials.truncate(2),
        |s| {
            s.trials.pop();
        },
        |s| s.trials[1].proposal_seed = s.trials[0].proposal_seed,
        |s| s.trials[1].evaluation_seeds = s.trials[0].evaluation_seeds.clone(),
        |s| s.trials[0].evaluation_seeds.truncate(1),
        |s| s.trials[0].evaluation_seeds = vec![0, 0],
        |s| s.attempts_per_trial = 0,
        |s| s.generation_cap = 0,
        |s| s.evaluation_cap = 0,
        |s| s.per_run_budget.max_tokens = 0,
        |s| s.efficiency_margin = f64::NAN,
        |s| s.efficiency_margin = -1.,
        |s| s.scoring.resamples = 1,
        |s| s.generation_cap = u64::MAX,
        |s| s.evaluation_cap = u64::MAX,
    ];
    for change in changes {
        let mut s = spec();
        change(&mut s);
        assert!(ExperimentProtocol::new(s).is_err());
    }
    let encoded = serde_json::to_value(spec()).unwrap();
    assert_eq!(
        serde_json::from_value::<ExperimentProtocolSpec>(encoded.clone()).unwrap(),
        spec()
    );
    let mut unknown = encoded;
    unknown["unregistered_field"] = true.into();
    assert!(serde_json::from_value::<ExperimentProtocolSpec>(unknown).is_err());
}

#[test]
fn hard_anchor_gates_and_no_accepted_proposals_prevent_positive_claims() {
    for mode in [Mode::HardGate, Mode::NoAccepted] {
        let report = execute(mode, EvidenceSource::Live).0;
        assert_ne!(report.verdict, ExperimentVerdict::SupportedImprovement);
        assert!(!report.ablations[0].anchors_accepted);
    }
}

#[test]
fn rejected_unresolved_and_failed_attempts_all_consume_the_same_caps() {
    let mut specification = spec();
    specification.attempts_per_trial = 4;
    let protocol = ExperimentProtocol::new(specification).unwrap();
    let mut host = Host {
        mode: Mode::MixedAttempts,
        requests: Vec::new(),
    };
    let report = run_experiment(&protocol, &mut host).unwrap();
    assert_eq!(report.attempts, 96);
    assert_eq!(report.generation_tokens, 960);
    assert_eq!(report.evaluation_tokens, 96);
    assert!(
        report
            .trials
            .iter()
            .all(|t| t.accepted == 1 && t.rejected == 1 && t.inconclusive == 1 && t.failed == 1)
    );
}

#[test]
fn pins_assignment_and_attempt_wire_contract_without_private_outputs() {
    let (_, host) = execute(Mode::Good, EvidenceSource::Scripted);
    let request = &host.requests[0];
    let encoded = serde_json::to_value(request).unwrap();
    assert_eq!(encoded["arm"], "Original");
    assert_eq!(encoded["ablation"], "Full");
    assert_eq!(encoded["attempts_per_trial"], 1);
    assert_eq!(encoded["per_run_budget"]["max_tokens"], 10);
    assert_eq!(
        serde_json::from_value::<TrialRequest>(encoded.clone()).unwrap(),
        *request
    );
    let mut unknown = encoded;
    unknown["oracle"] = true.into();
    assert!(serde_json::from_value::<TrialRequest>(unknown).is_err());
    for (status, tag) in [
        (AttemptStatus::Accepted, "Accepted"),
        (AttemptStatus::Rejected, "Rejected"),
        (AttemptStatus::Inconclusive, "Inconclusive"),
        (AttemptStatus::Failed, "Failed"),
    ] {
        let attempt = AttemptEvidence {
            status,
            generation_tokens: 3,
            evaluation_tokens: 7,
        };
        let encoded = serde_json::to_value(&attempt).unwrap();
        assert_eq!(encoded["status"], tag);
        assert_eq!(
            serde_json::from_value::<AttemptEvidence>(encoded).unwrap(),
            attempt
        );
    }
    for mode in [Mode::AllFail, Mode::WrongRequest] {
        let report = execute(mode, EvidenceSource::Live).0;
        let encoded = serde_json::to_string(&report).unwrap();
        assert_eq!(
            serde_json::from_str::<ExperimentReport>(&encoded).unwrap(),
            report
        );
        assert!(!encoded.contains("private-task-content"));
    }
}

#[test]
fn efficiency_scaling_does_not_overflow_a_finite_normalized_endpoint() {
    let mut specification = spec();
    specification.scoring.anchors[0].reference = 1e-306;
    let protocol = ExperimentProtocol::new(specification).unwrap();
    let mut host = Host {
        mode: Mode::Good,
        requests: Vec::new(),
    };
    let report = run_experiment(&protocol, &mut host).unwrap();
    assert!(
        report
            .ablations
            .iter()
            .all(|estimate| estimate.efficiency_delta.is_finite())
    );
    assert_eq!(report.verdict, ExperimentVerdict::SupportedImprovement);
}
