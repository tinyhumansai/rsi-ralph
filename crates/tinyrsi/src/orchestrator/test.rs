//! Regression tests for the host-gated proposal funnel.
#![expect(clippy::unwrap_used, reason = "assert test setup and outcomes")]
use super::*;
use crate::{
    Budget, Comparison, EvalDecision, EvalPolicy, EvalRun, Operation, Package, Patch, PatchClass,
    Prediction,
};
use tinyhivemind_lang::lineage::{Direction, ObjectiveScore, Verdict, VersionMetadata};

pub(super) fn baseline(archive: &mut Archive) {
    let mut package = Package::default();
    package.constitution.permitted_edits = vec![PatchClass::Context];
    package.manifest.context = vec!["context/main.md".into()];
    package.documents.insert(
        "context/main.md".into(),
        "<!-- item:method -->\nGuess an answer.\n".into(),
    );
    archive
        .record(
            package,
            VersionMetadata {
                parent: None,
                patch: Patch {
                    class: PatchClass::Context,
                    operations: vec![],
                },
                rationale: "baseline".into(),
                transcript_refs: vec![],
                scores: BTreeMap::from([(
                    "quality".into(),
                    ObjectiveScore {
                        value: 0.,
                        direction: Direction::Maximize,
                    },
                )]),
                verdict: Verdict::Accepted,
                memory_watermarks: BTreeMap::new(),
            },
        )
        .unwrap();
}
pub(super) fn proposal() -> Proposal {
    Proposal {
        title: "Verify".into(),
        patch: Patch {
            class: PatchClass::Context,
            operations: vec![Operation::Context {
                path: "context/main.md".into(),
                item: "method".into(),
                before: Some("Guess an answer.\n".into()),
                after: Some("Compute and check the answer.\n".into()),
            }],
        },
        mechanism: "Replace guessing with execution".into(),
        prediction: Prediction {
            metric: "verified_quality".into(),
            minimum_gain: 0.05,
            nodes: vec!["n1".into()],
        },
        transcript_refs: vec![1],
        risk: "extra tokens".into(),
        effort: "one delta".into(),
    }
}
pub(super) fn epoch() -> Epoch {
    Epoch {
        id: 0,
        class: PatchClass::Context,
        transcript_refs: vec![1],
        failure_nodes: vec!["n1".into()],
        budget: Budget { max_tokens: 100 },
        policy: EvalPolicy::default(),
    }
}
struct Harness {
    gain: f64,
    stages: Vec<EvalStage>,
    fail: bool,
    confirmation_gain: Option<f64>,
}
impl Evaluator for Harness {
    fn pin_memory(&mut self, inc: &CandidateState, _: &Patch) -> Result<BTreeMap<String, u64>> {
        Ok(inc.memory_watermarks.clone())
    }
    fn evaluate(
        &mut self,
        _: &CandidateState,
        _: &CandidateState,
        stage: EvalStage,
        _: &Budget,
    ) -> Result<EvalPair> {
        if self.fail {
            return Err(crate::Error::HostFailed);
        }
        self.stages.push(stage);
        let fresh = stage == EvalStage::Confirmation;
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
        Ok(EvalPair {
            incumbent: rows(0.2),
            candidate: rows(
                0.2 + if fresh {
                    self.confirmation_gain.unwrap_or(self.gain)
                } else {
                    self.gain
                },
            ),
        })
    }
}
struct Gate(bool);
impl ApprovalGate for Gate {
    fn approve(&mut self, _: &str, _: &Proposal, c: &Comparison) -> Result<bool> {
        assert!(matches!(c.decision, EvalDecision::Accept { .. }));
        Ok(self.0)
    }
}
fn harness(gain: f64) -> Harness {
    Harness {
        gain,
        stages: vec![],
        fail: false,
        confirmation_gain: None,
    }
}
#[test]
fn confirmed_gain_requires_human_approval_before_advancing_head() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path().join("archive.json")).unwrap();
    baseline(&mut a);
    let old = a.head().unwrap().id.clone();
    let mut h = harness(0.3);
    let out = run_proposal(&mut a, &epoch(), &proposal(), &mut h, &mut Gate(false)).unwrap();
    assert_eq!(out.reason, ReasonCode::ApprovalDenied);
    assert_eq!(a.head().unwrap().id, old);
    assert_eq!(a.records().len(), 2);
    assert_eq!(h.stages.len(), 3);
    assert!(out.apply_at.is_none());
}
#[test]
fn confirms_records_and_requests_turn_boundary_activation() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path().join("a.json")).unwrap();
    baseline(&mut a);
    let mut h = harness(0.3);
    let out = run_proposal(&mut a, &epoch(), &proposal(), &mut h, &mut Gate(true)).unwrap();
    assert_eq!(out.reason, ReasonCode::Accepted);
    assert_eq!(out.apply_at, Some(ApplyAt::NextTurn));
    assert_eq!(a.head().unwrap().id, out.candidate_id.unwrap());
    a.revert().unwrap();
    assert_eq!(a.records().len(), 2);
}
#[test]
fn rejects_ungrounded_and_guarded_proposals_before_execution() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path().join("a.json")).unwrap();
    baseline(&mut a);
    let mut h = harness(0.3);
    let mut p = proposal();
    p.transcript_refs = vec![99];
    assert_eq!(
        run_proposal(&mut a, &epoch(), &p, &mut h, &mut Gate(true))
            .unwrap()
            .reason,
        ReasonCode::Ungrounded
    );
    let mut p = proposal();
    p.patch.operations = vec![Operation::Json {
        pointer: "/constitution/max_turns".into(),
        before: Some(60.into()),
        after: Some(100.into()),
    }];
    assert_eq!(
        run_proposal(&mut a, &epoch(), &p, &mut h, &mut Gate(true))
            .unwrap()
            .reason,
        ReasonCode::Guard
    );
    assert_eq!(h.stages, []);
    assert_eq!(a.proposals().len(), 2);
}
#[test]
fn screen_stops_losers_and_duplicate_proposals_stay_in_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path().join("a.json")).unwrap();
    baseline(&mut a);
    let mut h = harness(-0.1);
    assert_eq!(
        run_proposal(&mut a, &epoch(), &proposal(), &mut h, &mut Gate(true))
            .unwrap()
            .reason,
        ReasonCode::NoGain
    );
    assert_eq!(h.stages, vec![EvalStage::Screen]);
    assert_eq!(
        run_proposal(&mut a, &epoch(), &proposal(), &mut h, &mut Gate(true))
            .unwrap()
            .reason,
        ReasonCode::Duplicate
    );
    assert_eq!(a.records().len(), 2);
}
#[test]
fn confirmation_failure_and_host_errors_do_not_promote() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path().join("a.json")).unwrap();
    baseline(&mut a);
    let mut h = harness(0.3);
    h.confirmation_gain = Some(0.);
    assert_eq!(
        run_proposal(&mut a, &epoch(), &proposal(), &mut h, &mut Gate(true))
            .unwrap()
            .reason,
        ReasonCode::NotReplicated
    );
    let mut p = proposal();
    if let Operation::Context { after, .. } = &mut p.patch.operations[0] {
        *after = Some("Another candidate\n".into());
    }
    h.fail = true;
    assert_eq!(
        run_proposal(&mut a, &epoch(), &p, &mut h, &mut Gate(true))
            .unwrap()
            .reason,
        ReasonCode::HostFailure
    );
    assert_eq!(a.records().len(), 2);
}

#[test]
fn confirmation_must_not_reuse_screening_tasks_or_seeds() {
    struct ReusedScreen(Harness);
    impl Evaluator for ReusedScreen {
        fn pin_memory(
            &mut self,
            inc: &CandidateState,
            patch: &Patch,
        ) -> Result<BTreeMap<String, u64>> {
            self.0.pin_memory(inc, patch)
        }
        fn evaluate(
            &mut self,
            inc: &CandidateState,
            cand: &CandidateState,
            stage: EvalStage,
            budget: &Budget,
        ) -> Result<EvalPair> {
            let mut pair = self.0.evaluate(inc, cand, stage, budget)?;
            for run in pair.incumbent.iter_mut().chain(pair.candidate.iter_mut()) {
                match stage {
                    EvalStage::Screen | EvalStage::Confirmation => {
                        run.task = "screen-task".into();
                        run.seed %= 10;
                    }
                    EvalStage::Selection => {
                        run.task = "selection-task".into();
                        run.seed += 20;
                    }
                }
            }
            Ok(pair)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path().join("a.json")).unwrap();
    baseline(&mut a);
    let out = run_proposal(
        &mut a,
        &epoch(),
        &proposal(),
        &mut ReusedScreen(harness(0.3)),
        &mut Gate(true),
    )
    .unwrap();
    assert_eq!(out.reason, ReasonCode::NotReplicated);
    assert_eq!(a.evaluations().len(), 3);
    assert_eq!(a.rejected().len(), 1);
}
#[test]
fn rejects_invalid_predictions_wrong_classes_and_stale_operations_without_models() {
    for case in 0..9 {
        let dir = tempfile::tempdir().unwrap();
        let mut a = Archive::open(dir.path().join("a.json")).unwrap();
        baseline(&mut a);
        let mut p = proposal();
        let expected = match case {
            0 => {
                p.prediction.minimum_gain = f64::NAN;
                ReasonCode::Untestable
            }
            1 => {
                p.prediction.minimum_gain = 0.;
                ReasonCode::Untestable
            }
            2 => {
                p.prediction.minimum_gain = 2.;
                ReasonCode::Untestable
            }
            3 => {
                p.prediction.metric = "self_claims".into();
                ReasonCode::Untestable
            }
            4 => {
                p.prediction.nodes.clear();
                ReasonCode::Untestable
            }
            5 => {
                p.patch.operations.clear();
                ReasonCode::Untestable
            }
            6 => {
                p.patch.class = PatchClass::Policy;
                ReasonCode::WrongClass
            }
            7 => {
                p.prediction.nodes = vec!["invented".into()];
                ReasonCode::Ungrounded
            }
            _ => {
                if let Operation::Context { before, .. } = &mut p.patch.operations[0] {
                    *before = Some("stale\n".into());
                }
                ReasonCode::Unformalizable
            }
        };
        let mut h = harness(0.3);
        assert_eq!(
            run_proposal(&mut a, &epoch(), &p, &mut h, &mut Gate(true))
                .unwrap()
                .reason,
            expected
        );
        assert_eq!(h.stages, []);
    }
}
#[test]
fn preserves_valid_unscored_snapshots_on_execution_and_approval_errors() {
    struct FailsGate;
    impl ApprovalGate for FailsGate {
        fn approve(&mut self, _: &str, _: &Proposal, _: &Comparison) -> Result<bool> {
            Err(crate::Error::HostFailed)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    let mut a = Archive::open(&path).unwrap();
    baseline(&mut a);
    let mut h = harness(0.3);
    h.fail = true;
    let out = run_proposal(&mut a, &epoch(), &proposal(), &mut h, &mut Gate(true)).unwrap();
    let id = out.candidate_id.unwrap();
    assert_eq!(out.reason, ReasonCode::HostFailure);
    assert!(a.package(&id).is_some());
    drop(a);
    let a = Archive::open(&path).unwrap();
    assert!(a.package(&id).is_some());
    let mut a = Archive::open(dir.path().join("b.json")).unwrap();
    baseline(&mut a);
    assert_eq!(
        run_proposal(
            &mut a,
            &epoch(),
            &proposal(),
            &mut harness(0.3),
            &mut FailsGate
        )
        .unwrap()
        .reason,
        ReasonCode::HostFailure
    );
    assert_eq!(a.rejected().len(), 1);
    assert_eq!(a.evaluations().len(), 3);
}
#[test]
fn an_uncertain_selection_never_requests_confirmation_or_promotion() {
    struct Noisy(Harness);
    impl Evaluator for Noisy {
        fn pin_memory(&mut self, inc: &CandidateState, p: &Patch) -> Result<BTreeMap<String, u64>> {
            self.0.pin_memory(inc, p)
        }
        fn evaluate(
            &mut self,
            inc: &CandidateState,
            cand: &CandidateState,
            s: EvalStage,
            b: &Budget,
        ) -> Result<EvalPair> {
            let mut pair = self.0.evaluate(inc, cand, s, b)?;
            for run in &mut pair.candidate {
                run.quality = match run.seed % 3 {
                    0 => 0.2,
                    1 => 0.4,
                    _ => 0.25,
                };
            }
            Ok(pair)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let mut a = Archive::open(dir.path().join("a.json")).unwrap();
    baseline(&mut a);
    let mut h = Noisy(harness(0.));
    assert_eq!(
        run_proposal(&mut a, &epoch(), &proposal(), &mut h, &mut Gate(true))
            .unwrap()
            .reason,
        ReasonCode::Inconclusive
    );
    assert_eq!(h.0.stages, vec![EvalStage::Screen, EvalStage::Selection]);
}

#[test]
fn each_patch_class_has_a_concrete_host_activation_boundary() {
    assert_eq!(apply_at(PatchClass::Context), ApplyAt::NextTurn);
    assert_eq!(apply_at(PatchClass::Memory), ApplyAt::NextTurn);
    assert_eq!(apply_at(PatchClass::Prompt), ApplyAt::ReplaceAgent);
    assert_eq!(apply_at(PatchClass::Roster), ApplyAt::ManageRoster);
    for class in [PatchClass::Topology, PatchClass::Policy, PatchClass::Goal] {
        assert_eq!(apply_at(class), ApplyAt::BetweenSessions);
    }
}

#[test]
fn cannot_unfreeze_read_only_memory_and_learn_in_one_patch() {
    use tinyhivemind_lang::{MemoryIdentity, MemoryKind, MemoryLifecycle};
    struct Pin;
    impl Evaluator for Pin {
        fn pin_memory(&mut self, _: &CandidateState, _: &Patch) -> Result<BTreeMap<String, u64>> {
            Ok(BTreeMap::from([("notes".into(), 3)]))
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
    let mut package = Package::default();
    package.constitution.permitted_edits = vec![PatchClass::Memory];
    package.memory.identities = vec![MemoryIdentity {
        id: "notes".into(),
        namespace: "team:hive/agent:notes".into(),
        kind: MemoryKind::Agent,
        lifecycle: MemoryLifecycle::Hive,
        read_only: true,
        fork_parent: None,
    }];
    let incumbent = CandidateState {
        package,
        memory_watermarks: BTreeMap::from([("notes".into(), 2)]),
    };
    let mut p = proposal();
    p.patch = Patch {
        class: PatchClass::Memory,
        operations: vec![
            Operation::Json {
                pointer: "/memory/identities/0/read_only".into(),
                before: Some(true.into()),
                after: Some(false.into()),
            },
            Operation::Learning {
                identity: "notes".into(),
                entry: "e1".into(),
                watermark: 2,
                before: None,
                after: Some("learned".into()),
            },
        ],
    };
    assert!(matches!(
        prepare(&incumbent, &p, &mut Pin),
        Err(ReasonCode::Guard)
    ));
}
