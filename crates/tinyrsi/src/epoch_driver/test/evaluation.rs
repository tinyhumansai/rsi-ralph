//! Synchronous normalized evaluation and restart-safe human gate receipts.
use super::*;
use crate::{
    CandidateState, EvalPair, EvalRun, EvalStage, Evaluator, HApprovalGate, HEvalPair, HEvaluator,
    ScoreComparison, ScoreHealth,
};
#[derive(Default)]
pub(super) struct Host {
    calls: u64,
}
impl Evaluator for Host {
    fn pin_memory(&mut self, inc: &CandidateState, _: &Patch) -> Result<BTreeMap<String, u64>> {
        Ok(inc.memory_watermarks.clone())
    }
    fn evaluate(
        &mut self,
        _: &CandidateState,
        _: &CandidateState,
        _: EvalStage,
        _: &Budget,
    ) -> Result<EvalPair> {
        Err(Error::HostFailed)
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
        self.calls += 1;
        let fresh = stage == EvalStage::Confirmation;
        let rows = |quality| {
            (0..3)
                .map(|seed| EvalRun {
                    task: if fresh { "fresh" } else { "old" }.into(),
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
        Ok(HEvalPair {
            pair: EvalPair {
                incumbent: rows(0.2),
                candidate: rows(0.6),
            },
            incumbent_health: health.clone(),
            candidate_health: health,
            suite_id: match stage {
                EvalStage::Screen => "s",
                EvalStage::Selection => "v",
                EvalStage::Confirmation => "c",
            }
            .into(),
        })
    }
}
pub(super) struct Human {
    pub(super) approve: bool,
    pub(super) calls: u64,
}
impl HApprovalGate for Human {
    fn approve_h(&mut self, _: &str, _: &Proposal, _: &ScoreComparison) -> Result<bool> {
        self.calls += 1;
        Ok(self.approve)
    }
}
#[test]
fn evaluation_reopens_without_repeating_private_draws_or_human_decision() {
    for approved in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
        let budget = budget();
        let calls = Arc::new(AtomicUsize::new(0));
        let memory = Arc::new(
            DurableRetreatMemory::open(
                dir.path().join("m"),
                "h",
                crate::MemoryLimits::default(),
                vec![],
            )
            .unwrap(),
        );
        let path = dir.path().join("j");
        let mut journal = EpochJournal::open(&path).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime
            .block_on(prepare_retreat_epoch(
                &mut journal,
                Arc::new(Script {
                    calls,
                    fail: false,
                    invalid: false,
                }),
                memory.clone(),
                inputs(
                    &archive,
                    &checkpoint,
                    &dossier,
                    &epoch,
                    GenerationMode::Retreat,
                    &budget,
                ),
            ))
            .unwrap();
        drop(runtime);
        let proposal_count = journal.prepared(1).unwrap().proposals.len();
        let priors = vec![Some(0.75); proposal_count];
        journal
            .register_proposal_priors(1, &priors, &archive)
            .unwrap();
        let mut host = Host::default();
        let mut human = Human {
            approve: approved,
            calls: 0,
        };
        let outcome = evaluate_retreat_epoch(
            &mut journal,
            &mut archive,
            &epoch,
            &mut host,
            &mut human,
            &memory,
        )
        .unwrap();
        assert!(outcome.complete);
        assert_eq!(
            outcome.candidates[0].reason,
            if approved {
                ReasonCode::Accepted
            } else {
                ReasonCode::ApprovalDenied
            }
        );
        assert_eq!(host.calls, 3);
        assert_eq!(human.calls, 1);
        let dataset = assert_selection_dataset(&mut journal, &archive, proposal_count, &priors);
        assert_eq!(memory.outcomes().unwrap().len(), 1);
        assert_eq!(
            archive.head().unwrap().id == checkpoint.deployed_id.clone().unwrap(),
            !approved
        );
        drop(journal);
        let mut journal = EpochJournal::open(&path).unwrap();
        let repeated = evaluate_retreat_epoch(
            &mut journal,
            &mut archive,
            &epoch,
            &mut host,
            &mut human,
            &memory,
        )
        .unwrap();
        assert_eq!(outcome, repeated);
        assert_eq!(
            journal.proposal_dataset(1, &archive).unwrap(),
            Some(dataset)
        );
        assert_eq!(host.calls, 3);
        assert_eq!(human.calls, 1);
    }
}
/// Checks that only actual selection measurements label the generated occurrences.
fn assert_selection_dataset(
    journal: &mut EpochJournal,
    archive: &Archive,
    proposal_count: usize,
    priors: &[Option<f64>],
) -> ProposalOutcomeDataset {
    let dataset = journal.proposal_dataset(1, archive).unwrap().unwrap();
    assert_eq!(dataset.rows.len(), proposal_count);
    assert_eq!(
        dataset.rows[0].selection.as_ref().unwrap().eligible,
        Some(true)
    );
    assert_eq!(dataset.rows[0].prior, Some(0.75));
    assert_eq!(
        dataset.rows[0].occurrence,
        Some(ProposalOccurrence {
            call_id: 1,
            item_index: 0
        })
    );
    assert_eq!(dataset.rows[0].stage, ProposalStage::Confirmation);
    assert_eq!(dataset.metrics(1).unwrap().brier, Some(0.0625));
    let public = serde_json::to_string(&dataset).unwrap();
    assert!(!public.contains("fresh"));
    assert!(!public.contains("Guess"));
    journal
        .register_proposal_priors(1, priors, archive)
        .unwrap();
    assert!(
        journal
            .register_proposal_priors(1, &vec![Some(0.25); proposal_count], archive)
            .is_err()
    );
    dataset
}
#[test]
fn interrupted_evaluation_never_replays_unknown_effects() {
    let dir = tempfile::tempdir().unwrap();
    let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let budget = budget();
    let memory = Arc::new(
        DurableRetreatMemory::open(
            dir.path().join("m"),
            "h",
            crate::MemoryLimits::default(),
            vec![],
        )
        .unwrap(),
    );
    let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime
        .block_on(prepare_retreat_epoch(
            &mut journal,
            Arc::new(Script {
                calls: Arc::new(AtomicUsize::new(0)),
                fail: false,
                invalid: false,
            }),
            memory.clone(),
            inputs(
                &archive,
                &checkpoint,
                &dossier,
                &epoch,
                GenerationMode::Direct,
                &budget,
            ),
        ))
        .unwrap();
    drop(runtime);
    let mut entry = journal.state.entries[&1].clone();
    entry.evaluating = true;
    journal.commit_entry(entry).unwrap();
    let mut host = Host::default();
    let mut human = Human {
        approve: true,
        calls: 0,
    };
    assert!(
        evaluate_retreat_epoch(
            &mut journal,
            &mut archive,
            &epoch,
            &mut host,
            &mut human,
            &memory
        )
        .is_err()
    );
    assert_eq!(host.calls, 0);
    assert_eq!(human.calls, 0);
}
#[test]
fn every_valid_formatter_occurrence_reaches_duplicate_funnel() {
    struct Five;
    impl ModelPort for Five {
        fn generate(&self, request: crate::ModelRequest) -> crate::ModelFuture {
            if request.system().starts_with("You propose.")
                || request.system().starts_with("You attack.")
                || request.system().starts_with("You are a strict reviewer")
                || request.system().starts_with("You are one of several")
            {
                return Script {
                    calls: Arc::new(AtomicUsize::new(0)),
                    fail: false,
                    invalid: false,
                }
                .generate(request);
            }
            Box::pin(async {
                Ok(crate::ModelReply {
                    content: serde_json::to_vec(&serde_json::json!({"items":vec![proposal();5]}))
                        .unwrap(),
                    input_tokens: 10,
                    output_tokens: 10,
                })
            })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let mut cap = budget();
    cap.variations = 1;
    cap.select_top_k = 5;
    let memory = Arc::new(
        DurableRetreatMemory::open(
            dir.path().join("m"),
            "h",
            crate::MemoryLimits::default(),
            vec![],
        )
        .unwrap(),
    );
    let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let prepared = runtime
        .block_on(prepare_retreat_epoch(
            &mut journal,
            Arc::new(Five),
            memory.clone(),
            inputs(
                &archive,
                &checkpoint,
                &dossier,
                &epoch,
                GenerationMode::Retreat,
                &cap,
            ),
        ))
        .unwrap();
    drop(runtime);
    assert_eq!(prepared.proposals.len(), 5);
    assert_eq!(prepared.ranked_items, 1);
    let mut host = Host::default();
    let mut human = Human {
        approve: false,
        calls: 0,
    };
    let outcome = evaluate_retreat_epoch(
        &mut journal,
        &mut archive,
        &epoch,
        &mut host,
        &mut human,
        &memory,
    )
    .unwrap();
    assert_eq!(outcome.candidates.len(), 5);
    assert_eq!(outcome.candidates[0].reason, ReasonCode::ApprovalDenied);
    assert!(
        outcome.candidates[1..]
            .iter()
            .all(|r| r.reason == ReasonCode::Duplicate)
    );
    assert_eq!(host.calls, 3);
}
