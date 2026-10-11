//! Synchronous normalized evaluation and restart-safe human gate receipts.
use super::*;
use crate::{
    CandidateState, EvalPair, EvalRun, EvalStage, Evaluator, HApprovalGate, HEvalPair, HEvaluator,
    LoopEvent, ScoreComparison, ScoreHealth, TraceSink,
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
        assert_epoch_trace(&journal, &archive, &outcome, approved);
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
    assert_duplicates_are_not_rescored(&archive, &prepared, &outcome);
}

/// One evaluation, so one measurement and one score; the duplicates repeat a
/// patch nothing re-ran, and each idea links exactly what it became.
fn assert_duplicates_are_not_rescored(
    archive: &Archive,
    prepared: &PreparedRetreatEpoch,
    outcome: &RetreatEpochOutcome,
) {
    let mut trace = crate::VecTrace::new(crate::ManualClock::new());
    record_epoch(&mut trace, 1, archive, prepared, outcome).unwrap();
    let events: Vec<LoopEvent> = trace.records().iter().map(|r| r.event.clone()).collect();
    let kinds: Vec<&str> = events.iter().map(LoopEvent::kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == "measured").count(), 1);
    assert_eq!(kinds.iter().filter(|k| **k == "scored").count(), 1);
    assert!(outcome.candidates[0].candidate_id.is_some());
    let linked: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            LoopEvent::Ideated { candidate, .. } => Some(candidate.clone()),
            _ => None,
        })
        .collect();
    let expected: Vec<_> = outcome
        .candidates
        .iter()
        .map(|c| c.candidate_id.clone())
        .collect();
    assert_eq!(linked, expected);
}

/// The epoch's proposals and outcome, recorded on the loop trace, read as the
/// visualizer expects: ideas, then measurement and score per candidate, an
/// improvement only when the human approved, and the epoch's end.
fn assert_epoch_trace(
    journal: &EpochJournal,
    archive: &Archive,
    outcome: &RetreatEpochOutcome,
    approved: bool,
) {
    use crate::{ManualClock, ScoreDecision, VecTrace};
    let prepared = journal.prepared(1).unwrap();
    let mut trace = VecTrace::new(ManualClock::new());
    record_epoch(&mut trace, 7, archive, prepared, outcome).unwrap();
    let events: Vec<LoopEvent> = trace.records().iter().map(|r| r.event.clone()).collect();
    let kinds: Vec<&str> = events.iter().map(LoopEvent::kind).collect();
    let ideas = prepared.proposals.len();
    assert_eq!(kinds[..ideas], vec!["ideated"; ideas][..]);
    let count = |kind: &str| kinds.iter().filter(|k| **k == kind).count();
    // Every candidate that ran is scored; only the evaluated one is measured,
    // and only an approved one improves the incumbent.
    let ran = outcome
        .candidates
        .iter()
        .filter(|c| c.candidate_id.is_some() && c.reason != ReasonCode::Duplicate)
        .count();
    assert_eq!(count("scored"), ran);
    assert_eq!(count("measured"), 1);
    assert_eq!(count("improved"), usize::from(approved));
    assert_eq!(kinds[ideas], "measured");
    assert_eq!(kinds.last(), Some(&"epoch_finished"));
    assert!(events.iter().all(|e| match e {
        LoopEvent::Ideated {
            epoch, proposal_id, ..
        } => *epoch == 7 && proposal_id.starts_with("p7-"),
        LoopEvent::Measured { epoch, .. }
        | LoopEvent::Scored { epoch, .. }
        | LoopEvent::Improved { epoch, .. }
        | LoopEvent::EpochFinished { epoch, .. } => *epoch == 7,
        _ => false,
    }));
    let scored = events
        .iter()
        .find_map(|e| match e {
            LoopEvent::Scored { decision, .. } => Some(*decision),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        scored,
        if approved {
            ScoreDecision::Accept
        } else {
            ScoreDecision::Reject
        }
    );
    // Every formalized proposal's idea names the candidate it became.
    for c in &outcome.candidates {
        let linked = events.iter().find_map(|e| match e {
            LoopEvent::Ideated {
                proposal_id,
                candidate,
                ..
            } if *proposal_id == format!("p7-{}", c.proposal_index) => Some(candidate.clone()),
            _ => None,
        });
        assert_eq!(linked, Some(c.candidate_id.clone()));
    }
    // The epoch ends on a metric: the accepted candidate's H, or the
    // incumbent's on the same private draw.
    let measured = events.iter().find_map(|e| match e {
        LoopEvent::Measured { mean, .. } => *mean,
        _ => None,
    });
    let finished = events.iter().find_map(|e| match e {
        LoopEvent::EpochFinished {
            incumbent_metric, ..
        } => Some(*incumbent_metric),
        _ => None,
    });
    let finished = finished.unwrap().unwrap();
    assert_eq!(approved, Some(finished) == measured);
}

/// A prepared epoch of `n` copies of the fixture proposal.
fn prepared(epoch: u64, n: usize) -> PreparedRetreatEpoch {
    PreparedRetreatEpoch {
        epoch,
        incumbent_id: "base".into(),
        mode: GenerationMode::Direct,
        fingerprint: "f".into(),
        policy_fingerprint: "p".into(),
        proposals: vec![proposal(); n],
        ranked_items: n,
        invalid_items: 0,
        meter: crate::GenerationMeter::default(),
    }
}

#[test]
fn an_unmaterialized_proposal_stays_candidate_less_on_the_trace() {
    use crate::{EpochCandidateOutcome, ManualClock, VecTrace};
    let dir = tempfile::tempdir().unwrap();
    let archive = Archive::open(dir.path().join("archive.json")).unwrap();
    let outcome = RetreatEpochOutcome {
        epoch: 3,
        candidates: vec![EpochCandidateOutcome {
            proposal_index: 2,
            reason: ReasonCode::Guard,
            candidate_id: None,
        }],
        complete: true,
    };
    let mut trace = VecTrace::new(ManualClock::new());
    record_epoch(&mut trace, 3, &archive, &prepared(3, 3), &outcome).unwrap();
    let events: Vec<LoopEvent> = trace.records().iter().map(|r| r.event.clone()).collect();
    let kinds: Vec<&str> = events.iter().map(LoopEvent::kind).collect();
    assert_eq!(kinds, ["ideated", "ideated", "ideated", "epoch_finished"]);
    assert!(events.iter().all(|e| match e {
        LoopEvent::Ideated { candidate, .. } => candidate.is_none(),
        LoopEvent::EpochFinished {
            incumbent,
            incumbent_metric,
            ..
        } => incumbent == "base" && incumbent_metric.is_none(),
        _ => false,
    }));
}

#[test]
fn a_failing_trace_sink_surfaces_as_a_trace_error() {
    use crate::{EpochCandidateOutcome, TraceError};
    struct Broken;
    impl TraceSink for Broken {
        fn record(&mut self, _: LoopEvent) -> std::result::Result<(), TraceError> {
            Err(TraceError::Io)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let archive = Archive::open(dir.path().join("archive.json")).unwrap();
    let outcome = RetreatEpochOutcome {
        epoch: 1,
        candidates: vec![EpochCandidateOutcome {
            proposal_index: 0,
            reason: ReasonCode::Guard,
            candidate_id: None,
        }],
        complete: true,
    };
    assert_eq!(
        record_epoch(&mut Broken, 1, &archive, &prepared(1, 1), &outcome),
        Err(Error::TraceIo)
    );
}
