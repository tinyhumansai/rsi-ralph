//! Normalized decisions, private-suite binding, guards, and archive replay.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixture outcomes")]
use super::super::test::{baseline, epoch, proposal};
use super::*;
use crate::scoring::ScoringPolicySpec;
use crate::{
    Archive, Budget, CandidateState, EvalDecision, EvalPair, EvalRun, EvalStage, Evaluator,
    FamilyAnchor, Patch, Proposal, ReasonCode, Result, ScoreComparison, ScoreHealth,
    ScorePenalties,
};
use std::collections::BTreeMap;

fn setup_epoch() -> HEpoch {
    HEpoch {
        epoch: epoch(),
        policy: ScoringPolicySpec {
            anchors: vec![FamilyAnchor {
                family: "code".into(),
                baseline: 0.0,
                reference: 0.5,
                weight: 1.0,
                regression_limit: 0.0,
            }],
            penalties: ScorePenalties {
                false_claims: 1.0,
                tamper: 1.0,
                cost: 1.0,
            },
            margin: 0.01,
            cost_tolerance: 0.0,
            resamples: 200,
            bootstrap_seed: 42,
        },
        suite_ids: BTreeMap::from([
            ("Screen".into(), "s".into()),
            ("Selection".into(), "v".into()),
            ("Confirmation".into(), "c".into()),
        ]),
    }
}
fn hproposal() -> Proposal {
    let mut p = proposal();
    p.prediction.metric = "normalized_h".into();
    p
}
#[derive(Default)]
enum Fault {
    #[default]
    None,
    Invalid,
    Reuse,
    Health,
    Suite,
    Noisy,
    ConfirmationNoisy,
    ConfirmationLoss,
    Cost,
    Tamper,
    Family,
    Pin,
}
#[derive(Default)]
struct Host {
    fail: Option<EvalStage>,
    fault: Fault,
    stages: Vec<EvalStage>,
}
impl Evaluator for Host {
    fn pin_memory(&mut self, inc: &CandidateState, _: &Patch) -> Result<BTreeMap<String, u64>> {
        if matches!(self.fault, Fault::Pin) {
            return Err(crate::Error::HostFailed);
        }
        Ok(inc.memory_watermarks.clone())
    }
    fn evaluate(
        &mut self,
        _: &CandidateState,
        _: &CandidateState,
        _: EvalStage,
        _: &Budget,
    ) -> Result<EvalPair> {
        Err(crate::Error::HostFailed)
    }
}
impl HEvaluator for Host {
    fn evaluate_h(
        &mut self,
        _: &CandidateState,
        _: &CandidateState,
        stage: EvalStage,
        _: &Budget,
    ) -> Result<HEvalPair> {
        self.stages.push(stage);
        if self.fail == Some(stage) {
            return Err(crate::Error::HostFailed);
        }
        let fresh = stage == EvalStage::Confirmation && !matches!(self.fault, Fault::Reuse);
        let rows = |quality| {
            (0..3)
                .map(|seed| EvalRun {
                    task: if fresh { "confirmation" } else { "selection" }.into(),
                    family: "code".into(),
                    seed: seed + if fresh { 10 } else { 0 },
                    quality,
                    tokens: 10,
                    false_claims: 0,
                    tamper: 0,
                })
                .collect()
        };
        let health = ScoreHealth {
            false_claim_rate: 0.0,
            tamper_rate: 0.0,
            cost_ratio: 1.0,
        };
        let mut pair = EvalPair {
            incumbent: rows(0.2),
            candidate: rows(0.6),
        };
        if matches!(self.fault, Fault::Noisy)
            || (stage == EvalStage::Confirmation && matches!(self.fault, Fault::ConfirmationNoisy))
        {
            for run in &mut pair.candidate {
                run.quality = match run.seed % 3 {
                    0 => 0.2,
                    1 => 0.4,
                    _ => 0.25,
                };
            }
        }
        if stage == EvalStage::Confirmation && matches!(self.fault, Fault::ConfirmationLoss) {
            for run in &mut pair.candidate {
                run.quality = 0.1;
            }
        }
        if matches!(self.fault, Fault::Cost) {
            for run in &mut pair.candidate {
                run.tokens = 11;
            }
        }
        if matches!(self.fault, Fault::Family) {
            for mut run in pair.incumbent.clone() {
                run.family = "research".into();
                run.task = format!("research-{}", run.task);
                pair.incumbent.push(run);
            }
            for mut run in pair.candidate.clone() {
                run.family = "research".into();
                run.task = format!("research-{}", run.task);
                run.quality = 0.1;
                pair.candidate.push(run);
            }
        }
        if matches!(self.fault, Fault::Invalid) {
            pair.candidate.pop();
        }
        Ok(HEvalPair {
            pair,
            incumbent_health: health.clone(),
            candidate_health: ScoreHealth {
                false_claim_rate: if matches!(self.fault, Fault::Health) {
                    0.1
                } else {
                    0.0
                },
                tamper_rate: if matches!(self.fault, Fault::Tamper) {
                    0.1
                } else {
                    0.0
                },
                cost_ratio: if matches!(self.fault, Fault::Cost) {
                    1.1
                } else {
                    1.0
                },
            },
            suite_id: if matches!(self.fault, Fault::Suite) {
                "wrong"
            } else {
                match stage {
                    EvalStage::Screen => "s",
                    EvalStage::Selection => "v",
                    EvalStage::Confirmation => "c",
                }
            }
            .into(),
        })
    }
}
struct Gate {
    approved: bool,
    fail: bool,
    calls: u32,
}
impl HApprovalGate for Gate {
    fn approve_h(&mut self, _: &str, p: &Proposal, c: &ScoreComparison) -> Result<bool> {
        self.calls += 1;
        assert_eq!(p.prediction.metric, "normalized_h");
        assert!(matches!(c.decision, EvalDecision::Accept { .. }));
        if self.fail {
            Err(crate::Error::HostFailed)
        } else {
            Ok(self.approved)
        }
    }
}
fn gate(approved: bool) -> Gate {
    Gate {
        approved,
        fail: false,
        calls: 0,
    }
}

#[test]
fn normalized_confirmation_is_required_before_human_gate_and_promotion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    let mut a = Archive::open(&path).unwrap();
    baseline(&mut a);
    let mut host = Host::default();
    let mut human = gate(true);
    let outcome =
        run_proposal_h(&mut a, &setup_epoch(), &hproposal(), &mut host, &mut human).unwrap();
    assert_eq!(outcome.reason, ReasonCode::Accepted);
    assert_eq!(human.calls, 1);
    assert_eq!(host.stages.len(), 3);
    assert!((outcome.comparison.unwrap().delta - 0.8).abs() < 1e-12);
    assert!((a.head().unwrap().scores["normalized_h"].value - 1.2).abs() < 1e-12);
    assert_eq!(a.normalized_evaluations().len(), 3);
    drop(a);
    assert_eq!(Archive::open(&path).unwrap().records().len(), 2);
}
#[test]
fn denied_or_failed_approval_does_not_advance_head() {
    for fail in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut a = Archive::open(dir.path().join("a.json")).unwrap();
        baseline(&mut a);
        let old = a.head().unwrap().id.clone();
        let mut human = gate(false);
        human.fail = fail;
        let outcome = run_proposal_h(
            &mut a,
            &setup_epoch(),
            &hproposal(),
            &mut Host::default(),
            &mut human,
        )
        .unwrap();
        assert_eq!(
            outcome.reason,
            if fail {
                ReasonCode::HostFailure
            } else {
                ReasonCode::ApprovalDenied
            }
        );
        assert_eq!(a.head().unwrap().id, old);
        assert_eq!(a.rejected().len(), 1);
    }
}
#[test]
fn mismatched_suite_invalid_evidence_and_host_errors_fail_closed() {
    for case in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let mut a = Archive::open(dir.path().join("a.json")).unwrap();
        baseline(&mut a);
        let mut host = Host::default();
        match case {
            0 => host.fault = Fault::Suite,
            1 => host.fault = Fault::Invalid,
            2 => host.fail = Some(EvalStage::Screen),
            _ => host.fail = Some(EvalStage::Selection),
        }
        let mut human = gate(true);
        let outcome =
            run_proposal_h(&mut a, &setup_epoch(), &hproposal(), &mut host, &mut human).unwrap();
        assert_eq!(outcome.reason, ReasonCode::HostFailure);
        assert_eq!(human.calls, 0);
        assert!(
            a.package(outcome.candidate_id.as_deref().unwrap())
                .is_some()
        );
    }
}
#[test]
fn raw_predictions_reused_confirmation_and_health_regressions_are_rejected() {
    for case in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let mut a = Archive::open(dir.path().join("a.json")).unwrap();
        baseline(&mut a);
        let mut host = Host::default();
        let mut p = hproposal();
        let expected = match case {
            0 => {
                p.prediction.metric = "verified_quality".into();
                ReasonCode::Untestable
            }
            1 => {
                host.fault = Fault::Reuse;
                ReasonCode::NotReplicated
            }
            _ => {
                host.fault = Fault::Health;
                ReasonCode::NoGain
            }
        };
        let outcome =
            run_proposal_h(&mut a, &setup_epoch(), &p, &mut host, &mut gate(true)).unwrap();
        assert_eq!(outcome.reason, expected);
    }
}

#[test]
fn normalized_configuration_and_prediction_validation_precede_execution() {
    for case in 0..7 {
        let dir = tempfile::tempdir().unwrap();
        let mut a = Archive::open(dir.path().join("a.json")).unwrap();
        baseline(&mut a);
        let mut e = setup_epoch();
        let mut p = hproposal();
        let mut host = Host::default();
        match case {
            0 => {
                e.suite_ids.remove("Confirmation");
            }
            1 => {
                e.suite_ids.insert("Confirmation".into(), "s".into());
            }
            2 => e.epoch.budget.max_tokens = 0,
            3 => e.policy.resamples = 199,
            4 => p.prediction.minimum_gain = f64::NAN,
            5 => p.prediction.minimum_gain = 0.0,
            _ => p.patch.class = crate::PatchClass::Memory,
        }
        let result = run_proposal_h(&mut a, &e, &p, &mut host, &mut gate(true));
        if case < 4 {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap().reason,
                if case == 6 {
                    ReasonCode::WrongClass
                } else {
                    ReasonCode::Untestable
                }
            );
        }
        assert_eq!(host.stages, []);
    }
}

#[test]
fn duplicate_and_guarded_normalized_attempts_keep_their_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path().join("a.json")).unwrap();
    baseline(&mut a);
    let mut host = Host {
        fault: Fault::Health,
        ..Host::default()
    };
    assert_eq!(
        run_proposal_h(
            &mut a,
            &setup_epoch(),
            &hproposal(),
            &mut host,
            &mut gate(true)
        )
        .unwrap()
        .reason,
        ReasonCode::NoGain
    );
    assert_eq!(
        run_proposal_h(
            &mut a,
            &setup_epoch(),
            &hproposal(),
            &mut host,
            &mut gate(true)
        )
        .unwrap()
        .reason,
        ReasonCode::Duplicate
    );
    let mut p = hproposal();
    p.patch.operations = vec![crate::Operation::Json {
        pointer: "/constitution/max_turns".into(),
        before: Some(60.into()),
        after: Some(100.into()),
    }];
    assert_eq!(
        run_proposal_h(&mut a, &setup_epoch(), &p, &mut host, &mut gate(true))
            .unwrap()
            .reason,
        ReasonCode::Guard
    );
    assert_eq!(a.proposals().len(), 3);
}

#[test]
fn replay_rejects_altered_bootstrap_and_normalized_policy() {
    for field in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.json");
        let mut a = Archive::open(&path).unwrap();
        baseline(&mut a);
        run_proposal_h(
            &mut a,
            &setup_epoch(),
            &hproposal(),
            &mut Host::default(),
            &mut gate(true),
        )
        .unwrap();
        drop(a);
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let event = json["events"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|event| event["event"] == "normalized_evaluation")
            .unwrap();
        match field {
            0 => event["evaluation"]["comparison"]["delta"] = 99.0.into(),
            1 => event["evaluation"]["policy"]["resamples"] = 199.into(),
            _ => event["evaluation"]["suite_id"] = "changed".into(),
        }
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(matches!(
            Archive::open(&path),
            Err(crate::Error::ArchiveCorrupt)
        ));
    }
}

#[test]
fn h_uncertainty_and_all_health_cost_gates_prevent_promotion() {
    for (fault, expected) in [
        (Fault::Noisy, ReasonCode::Inconclusive),
        (Fault::ConfirmationNoisy, ReasonCode::NotReplicated),
        (Fault::ConfirmationLoss, ReasonCode::NotReplicated),
        (Fault::Cost, ReasonCode::NoGain),
        (Fault::Tamper, ReasonCode::NoGain),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::open(dir.path().join("a.json")).unwrap();
        baseline(&mut archive);
        let old = archive.head().unwrap().id.clone();
        let mut host = Host {
            fault,
            ..Host::default()
        };
        let mut human = gate(true);
        let result = run_proposal_h(
            &mut archive,
            &setup_epoch(),
            &hproposal(),
            &mut host,
            &mut human,
        )
        .unwrap();
        assert_eq!(result.reason, expected);
        assert_eq!(human.calls, 0);
        assert_eq!(archive.head().unwrap().id, old);
        assert_eq!(archive.rejected().len(), 1);
    }
}

#[test]
fn negative_upper_h_stops_screening_when_all_hard_gates_pass() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    let mut archive = Archive::open(&path).unwrap();
    baseline(&mut archive);
    let old = archive.head().unwrap().id.clone();
    let mut epoch = setup_epoch();
    epoch.policy.cost_tolerance = 0.2;
    epoch.policy.penalties.cost = 10.0;
    let mut host = Host {
        fault: Fault::Cost,
        ..Host::default()
    };
    let mut human = gate(true);
    let result = run_proposal_h(&mut archive, &epoch, &hproposal(), &mut host, &mut human).unwrap();
    let comparison = result.comparison.unwrap();
    assert!(comparison.upper < 0.0);
    assert!(comparison.candidate.per_family["code"] > comparison.incumbent.per_family["code"]);
    assert!(comparison.candidate.health.cost_ratio <= 1.0 + epoch.policy.cost_tolerance);
    assert_eq!(comparison.candidate.health.false_claim_rate, 0.0);
    assert_eq!(comparison.candidate.health.tamper_rate, 0.0);
    assert_eq!(
        comparison.decision,
        EvalDecision::Reject {
            reason: "paired H interval rules out improvement".into()
        }
    );
    assert_eq!(result.reason, ReasonCode::NoGain);
    assert_eq!(host.stages, [EvalStage::Screen]);
    assert_eq!(human.calls, 0);
    assert_eq!(archive.head().unwrap().id, old);
    drop(archive);
    let reopened = Archive::open(path).unwrap();
    assert_eq!(reopened.head().unwrap().id, old);
    assert_eq!(reopened.normalized_evaluations().len(), 1);
    assert!(reopened.normalized_evaluations()[0].comparison.upper < 0.0);
}

#[test]
fn aggregate_h_gain_cannot_hide_a_family_regression() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = Archive::open(dir.path().join("a.json")).unwrap();
    baseline(&mut archive);
    let mut epoch = setup_epoch();
    let mut anchor = epoch.policy.anchors[0].clone();
    anchor.family = "research".into();
    epoch.policy.anchors.push(anchor);
    let outcome = run_proposal_h(
        &mut archive,
        &epoch,
        &hproposal(),
        &mut Host {
            fault: Fault::Family,
            ..Host::default()
        },
        &mut gate(true),
    )
    .unwrap();
    assert_eq!(outcome.reason, ReasonCode::NoGain);
    assert!(outcome.comparison.unwrap().delta > 0.0);
}

#[test]
fn pin_failure_and_prepared_duplicates_never_execute_h_tasks() {
    for pin_failure in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::open(dir.path().join("a.json")).unwrap();
        baseline(&mut archive);
        if !pin_failure {
            let head = archive.head().unwrap();
            let package = archive.package(&head.id).unwrap();
            let prepared = crate::tinyhivemind_lang::patch::apply(package, &hproposal().patch)
                .unwrap()
                .candidate;
            archive.prepare(prepared, BTreeMap::new()).unwrap();
        }
        let mut host = Host {
            fault: if pin_failure { Fault::Pin } else { Fault::None },
            ..Host::default()
        };
        let outcome = run_proposal_h(
            &mut archive,
            &setup_epoch(),
            &hproposal(),
            &mut host,
            &mut gate(true),
        )
        .unwrap();
        assert_eq!(
            outcome.reason,
            if pin_failure {
                ReasonCode::HostFailure
            } else {
                ReasonCode::Duplicate
            }
        );
        assert_eq!(host.stages, []);
    }
}

struct PreregistrationHost {
    host: Host,
    path: std::path::PathBuf,
    pinned: bool,
}
impl Evaluator for PreregistrationHost {
    fn pin_memory(&mut self, inc: &CandidateState, patch: &Patch) -> Result<BTreeMap<String, u64>> {
        let snapshot: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap();
        let events = snapshot["events"].as_array().unwrap();
        assert!(
            events
                .iter()
                .any(|e| e["event"] == "normalized_declaration")
        );
        assert!(!events.iter().any(|e| e["event"] == "prepared"));
        self.pinned = true;
        self.host.pin_memory(inc, patch)
    }
    fn evaluate(
        &mut self,
        _: &CandidateState,
        _: &CandidateState,
        _: EvalStage,
        _: &Budget,
    ) -> Result<EvalPair> {
        Err(crate::Error::HostFailed)
    }
}
impl HEvaluator for PreregistrationHost {
    fn evaluate_h(
        &mut self,
        inc: &CandidateState,
        cand: &CandidateState,
        stage: EvalStage,
        budget: &Budget,
    ) -> Result<HEvalPair> {
        assert!(self.pinned);
        self.host.evaluate_h(inc, cand, stage, budget)
    }
}

#[test]
fn normalized_configuration_is_durable_before_pinning_or_evaluation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    let mut a = Archive::open(&path).unwrap();
    baseline(&mut a);
    let mut host = PreregistrationHost {
        host: Host::default(),
        path: path.clone(),
        pinned: false,
    };
    let epoch = setup_epoch();
    let proposal = hproposal();
    let result = run_proposal_h(&mut a, &epoch, &proposal, &mut host, &mut gate(true)).unwrap();
    assert_eq!(result.reason, ReasonCode::Accepted);
    let declaration = a.normalized_declarations()[0];
    assert_eq!(declaration.policy, epoch.policy);
    assert_eq!(declaration.suite_ids, epoch.suite_ids);
    drop(a);
    assert_eq!(
        Archive::open(&path)
            .unwrap()
            .normalized_declarations()
            .len(),
        1
    );
    let mut snapshot: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let events = snapshot["events"].as_array_mut().unwrap();
    let index = events
        .iter()
        .position(|e| e["event"] == "normalized_declaration")
        .unwrap();
    let declaration = events.remove(index);
    let prepared = events
        .iter()
        .position(|e| e["event"] == "prepared")
        .unwrap();
    events.insert(prepared + 1, declaration);
    std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    assert!(matches!(
        Archive::open(&path),
        Err(crate::Error::ArchiveCorrupt)
    ));
}

#[test]
fn duplicate_dispositions_distinguish_accepted_and_rejected_host_history() {
    for approved in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("archive.json");
        let mut archive = Archive::open(&path).unwrap();
        baseline(&mut archive);
        let mut host = Host::default();
        run_proposal_h(
            &mut archive,
            &setup_epoch(),
            &hproposal(),
            &mut host,
            &mut gate(approved),
        )
        .unwrap();
        host.stages.clear();
        let repeated = run_proposal_h(
            &mut archive,
            &setup_epoch(),
            &hproposal(),
            &mut host,
            &mut gate(true),
        )
        .unwrap();
        assert_eq!(repeated.reason, ReasonCode::Duplicate);
        assert_eq!(host.stages, [] as [EvalStage; 0]);
        let report = archive.proposal_dispositions();
        let expected = if approved {
            crate::DuplicateDisposition::Accepted
        } else {
            crate::DuplicateDisposition::Rejected
        };
        assert_eq!(report.rows.last().unwrap().duplicate, Some(expected));
        drop(archive);
        assert_eq!(Archive::open(path).unwrap().proposal_dispositions(), report);
    }
}
