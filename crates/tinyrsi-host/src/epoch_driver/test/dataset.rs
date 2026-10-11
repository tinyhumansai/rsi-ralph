//! Actual epoch provenance, prior freezing, censoring and private archive projections.
use super::evaluation::{Host, Human};
use super::*;
use crate::{
    CandidateState, EvalPair, EvalStage, Evaluator, HEvalPair, HEvaluator, ModelIdentity,
    ProposalLabel,
};
struct Named {
    proposals: Vec<Proposal>,
    model: Option<ModelIdentity>,
    calls: Arc<AtomicUsize>,
}
impl ModelPort for Named {
    fn identity(&self) -> Option<ModelIdentity> {
        self.model.clone()
    }
    fn generate(&self, _: crate::ModelRequest) -> crate::ModelFuture {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let content = serde_json::to_vec(&serde_json::json!({"items":self.proposals})).unwrap();
        Box::pin(async move {
            Ok(crate::ModelReply {
                content,
                input_tokens: 10,
                output_tokens: 10,
            })
        })
    }
}
fn memory(path: &std::path::Path) -> Arc<DurableRetreatMemory> {
    Arc::new(
        DurableRetreatMemory::open(
            path,
            "h",
            crate::MemoryLimits::default(),
            vec!["private-canary".into()],
        )
        .unwrap(),
    )
}
fn prepare(
    journal: &mut EpochJournal,
    archive: &Archive,
    checkpoint: &DeploymentCheckpoint,
    dossier: &Dossier,
    epoch: &HEpoch,
    memory: Arc<DurableRetreatMemory>,
    port: Arc<dyn ModelPort>,
) -> Result<PreparedRetreatEpoch> {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(prepare_retreat_epoch(
            journal,
            port,
            memory,
            inputs(
                archive,
                checkpoint,
                dossier,
                epoch,
                GenerationMode::Direct,
                &budget(),
            ),
        ))
}
fn evaluate(
    journal: &mut EpochJournal,
    archive: &mut Archive,
    epoch: &HEpoch,
    memory: &DurableRetreatMemory,
    approved: bool,
) {
    evaluate_retreat_epoch(
        journal,
        archive,
        epoch,
        &mut Host::default(),
        &mut Human {
            approve: approved,
            calls: 0,
        },
        memory,
    )
    .unwrap();
}
#[test]
fn actual_model_seed_and_occurrences_survive_reopen_and_changed_models_cannot_reuse_generation() {
    let dir = tempfile::tempdir().unwrap();
    let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let model = ModelIdentity {
        provider: "test-provider".into(),
        model: "frozen-v1".into(),
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let port = Arc::new(Named {
        proposals: vec![proposal(), proposal()],
        model: Some(model.clone()),
        calls: calls.clone(),
    });
    let path = dir.path().join("j");
    let mut journal = EpochJournal::open(&path).unwrap();
    let memory = memory(&dir.path().join("m"));
    prepare(
        &mut journal,
        &archive,
        &checkpoint,
        &dossier,
        &epoch,
        memory.clone(),
        port.clone(),
    )
    .unwrap();
    let dataset = journal.proposal_dataset(1, &archive).unwrap().unwrap();
    assert_eq!(dataset.model, Some(model));
    assert_eq!(dataset.generation_seed, Some(42));
    assert_eq!(
        dataset.rows[1].occurrence,
        Some(ProposalOccurrence {
            call_id: 1,
            item_index: 1
        })
    );
    assert!(
        dataset
            .rows
            .iter()
            .all(|r| r.label == ProposalLabel::Pending)
    );
    drop(journal);
    let mut journal = EpochJournal::open(path).unwrap();
    assert_eq!(
        journal.proposal_dataset(1, &archive).unwrap(),
        Some(dataset)
    );
    prepare(
        &mut journal,
        &archive,
        &checkpoint,
        &dossier,
        &epoch,
        memory.clone(),
        port,
    )
    .unwrap();
    let changed = Arc::new(Named {
        proposals: vec![proposal()],
        model: None,
        calls: calls.clone(),
    });
    assert!(matches!(
        prepare(
            &mut journal,
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            memory,
            changed
        ),
        Err(Error::InvalidProposal)
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
#[test]
fn public_model_identifiers_are_validated_before_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let memory = memory(&dir.path().join("m"));
    let calls = Arc::new(AtomicUsize::new(0));
    for (i, label) in ["", "private-canary", "contains spaces", &"x".repeat(129)]
        .iter()
        .enumerate()
    {
        let mut journal = EpochJournal::open(dir.path().join(format!("j{i}"))).unwrap();
        let port = Arc::new(Named {
            proposals: vec![proposal()],
            model: Some(ModelIdentity {
                provider: "test".into(),
                model: (*label).into(),
            }),
            calls: calls.clone(),
        });
        assert!(
            prepare(
                &mut journal,
                &archive,
                &checkpoint,
                &dossier,
                &epoch,
                memory.clone(),
                port
            )
            .is_err()
        );
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}
#[test]
fn accepted_candidates_censor_later_occurrences_and_denials_do_not_label_duplicates() {
    for approved in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
        let memory = memory(&dir.path().join("m"));
        let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
        let port = Arc::new(Named {
            proposals: vec![proposal(); 3],
            model: None,
            calls: Arc::new(AtomicUsize::new(0)),
        });
        prepare(
            &mut journal,
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            memory.clone(),
            port,
        )
        .unwrap();
        journal
            .register_proposal_priors(1, &[Some(0.8); 3], &archive)
            .unwrap();
        evaluate(&mut journal, &mut archive, &epoch, &memory, approved);
        let dataset = journal.proposal_dataset(1, &archive).unwrap().unwrap();
        if !approved && let Some(destination) = std::env::var_os("TINYRSI_OUTCOME_DEMO") {
            let destination = std::path::PathBuf::from(destination);
            std::fs::create_dir_all(&destination).unwrap();
            std::fs::copy(&journal.path, destination.join("journal.json")).unwrap();
            std::fs::copy(
                super::super::receipts::path(&journal.path, 1),
                destination.join("journal.json.generation-1.json"),
            )
            .unwrap();
            std::fs::copy(dir.path().join("a"), destination.join("archive.json")).unwrap();
        }
        for row in &dataset.rows[1..] {
            assert_eq!(
                row.label,
                if approved {
                    ProposalLabel::Censored
                } else {
                    ProposalLabel::Duplicate
                }
            );
            assert!(row.selection.is_none());
            assert_eq!(row.stage, ProposalStage::Generated);
        }
        let metrics = dataset.metrics(3).unwrap();
        assert_eq!(metrics.brier_pairs, 1);
        assert_eq!(metrics.missing_labels, 2);
        assert!(metrics.precision_at_k.is_none());
    }
}
#[test]
fn priors_reject_invalid_values_changes_and_retroactive_external_evaluation() {
    let dir = tempfile::tempdir().unwrap();
    let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let memory = memory(&dir.path().join("m"));
    let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
    assert!(journal.register_proposal_priors(1, &[], &archive).is_err());
    assert!(journal.proposal_dataset(1, &archive).unwrap().is_none());
    let port = Arc::new(Named {
        proposals: vec![proposal()],
        model: None,
        calls: Arc::new(AtomicUsize::new(0)),
    });
    prepare(
        &mut journal,
        &archive,
        &checkpoint,
        &dossier,
        &epoch,
        memory.clone(),
        port,
    )
    .unwrap();
    for priors in [
        vec![],
        vec![Some(-0.1)],
        vec![Some(f64::NAN)],
        vec![Some(1.1)],
    ] {
        assert!(
            journal
                .register_proposal_priors(1, &priors, &archive)
                .is_err()
        );
    }
    crate::run_proposal_h(
        &mut archive,
        &epoch,
        &proposal(),
        &mut Host::default(),
        &mut Human {
            approve: false,
            calls: 0,
        },
    )
    .unwrap();
    assert!(
        journal
            .register_proposal_priors(1, &[Some(0.5)], &archive)
            .is_err()
    );
}
#[test]
fn interrupted_duplicate_does_not_inherit_an_earlier_occurrence_measurement() {
    let dir = tempfile::tempdir().unwrap();
    let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let memory = memory(&dir.path().join("m"));
    let path = dir.path().join("j");
    let mut journal = EpochJournal::open(&path).unwrap();
    prepare(
        &mut journal,
        &archive,
        &checkpoint,
        &dossier,
        &epoch,
        memory.clone(),
        Arc::new(Named {
            proposals: vec![proposal(); 2],
            model: None,
            calls: Arc::new(AtomicUsize::new(0)),
        }),
    )
    .unwrap();
    evaluate(&mut journal, &mut archive, &epoch, &memory, false);
    let mut entry = journal.state.entries[&1].clone();
    entry.outcome.candidates.truncate(1);
    entry.outcome.complete = false;
    entry.evaluating = true;
    journal.commit_entry(entry).unwrap();
    drop(journal);
    let journal = EpochJournal::open(path).unwrap();
    let dataset = journal.proposal_dataset(1, &archive).unwrap().unwrap();
    assert_eq!(dataset.rows[0].label, ProposalLabel::Observed);
    assert_eq!(dataset.rows[1].label, ProposalLabel::Interrupted);
    assert!(dataset.rows[1].selection.is_none());
    assert!(dataset.rows[1].candidate_id.is_none());
    assert_eq!(dataset.metrics(2).unwrap().gain_pairs, 1);
}

#[test]
fn corrupt_final_reasons_cannot_override_observed_approval_or_stage_decisions() {
    for (approved, corrupt) in [
        (false, ReasonCode::Accepted),
        (true, ReasonCode::ApprovalDenied),
        (false, ReasonCode::NoGain),
        (false, ReasonCode::Inconclusive),
        (false, ReasonCode::NotReplicated),
        (false, ReasonCode::Duplicate),
        (false, ReasonCode::HostFailure),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
        let memory = memory(&dir.path().join("m"));
        let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
        prepare(
            &mut journal,
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            memory.clone(),
            Arc::new(Named {
                proposals: vec![proposal(); 3],
                model: None,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
        )
        .unwrap();
        evaluate(&mut journal, &mut archive, &epoch, &memory, approved);
        let mut entry = journal.state.entries[&1].clone();
        entry.outcome.candidates[0].reason = corrupt;
        journal.commit_entry(entry).unwrap();
        assert!(journal.proposal_dataset(1, &archive).is_err());
    }
}
struct Stages {
    fail: Option<EvalStage>,
    selection: Vec<f64>,
    confirmation: f64,
}
#[test]
fn prefilter_only_external_attempts_also_freeze_probability_registration() {
    let dir = tempfile::tempdir().unwrap();
    let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let memory = memory(&dir.path().join("m"));
    let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
    prepare(
        &mut journal,
        &archive,
        &checkpoint,
        &dossier,
        &epoch,
        memory,
        Arc::new(Named {
            proposals: vec![proposal()],
            model: None,
            calls: Arc::new(AtomicUsize::new(0)),
        }),
    )
    .unwrap();
    let mut wrong = epoch.clone();
    wrong.epoch.class = PatchClass::Prompt;
    let result = crate::run_proposal_h(
        &mut archive,
        &wrong,
        &proposal(),
        &mut Host::default(),
        &mut Human {
            approve: false,
            calls: 0,
        },
    )
    .unwrap();
    assert_eq!(result.reason, ReasonCode::WrongClass);
    assert_eq!(archive.normalized_declarations().len(), 0);
    assert_eq!(archive.normalized_evaluations().len(), 0);
    assert!(
        journal
            .register_proposal_priors(1, &[Some(0.5)], &archive)
            .is_err()
    );
}
#[test]
fn actual_rejection_receipts_match_their_observed_terminal_stage() {
    for (reason, quality, confirmation) in [
        (ReasonCode::NoGain, vec![0.0; 3], 0.6),
        (ReasonCode::Inconclusive, vec![0.2, 0.6, 0.2], 0.6),
        (ReasonCode::NotReplicated, vec![0.6; 3], 0.0),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
        let memory = memory(&dir.path().join("m"));
        let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
        prepare(
            &mut journal,
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            memory.clone(),
            Arc::new(Named {
                proposals: vec![proposal()],
                model: None,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
        )
        .unwrap();
        evaluate_retreat_epoch(
            &mut journal,
            &mut archive,
            &epoch,
            &mut Stages {
                fail: None,
                selection: quality,
                confirmation,
            },
            &mut Human {
                approve: false,
                calls: 0,
            },
            &memory,
        )
        .unwrap();
        let dataset = journal.proposal_dataset(1, &archive).unwrap().unwrap();
        assert_eq!(dataset.rows[0].reason, Some(reason));
        if reason != ReasonCode::Inconclusive {
            let mut entry = journal.state.entries[&1].clone();
            entry.outcome.candidates[0].reason = ReasonCode::Inconclusive;
            journal.commit_entry(entry).unwrap();
            assert!(journal.proposal_dataset(1, &archive).is_err());
        }
    }
}
impl Evaluator for Stages {
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
impl HEvaluator for Stages {
    fn evaluate_h(
        &mut self,
        inc: &CandidateState,
        cand: &CandidateState,
        stage: EvalStage,
        cap: &Budget,
    ) -> Result<HEvalPair> {
        if self.fail == Some(stage) {
            return Err(Error::HostFailed);
        }
        let mut pair = Host::default().evaluate_h(inc, cand, stage, cap)?;
        for (i, row) in pair.pair.candidate.iter_mut().enumerate() {
            if stage == EvalStage::Selection {
                row.quality = self.selection[i];
            }
            if stage == EvalStage::Confirmation {
                row.quality = self.confirmation;
            }
        }
        Ok(pair)
    }
}
#[test]
fn observed_stages_and_selection_labels_remain_distinct_from_confirmation_and_interruption() {
    for (index, fail, quality, expected) in [
        (0, None, vec![0.6; 3], ProposalLabel::Observed),
        (1, None, vec![0.0; 3], ProposalLabel::Observed),
        (2, None, vec![0.2, 0.6, 0.2], ProposalLabel::Inconclusive),
        (
            3,
            Some(EvalStage::Selection),
            vec![0.6; 3],
            ProposalLabel::Interrupted,
        ),
        (
            4,
            Some(EvalStage::Confirmation),
            vec![0.6; 3],
            ProposalLabel::Observed,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
        let memory = memory(&dir.path().join("m"));
        let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
        prepare(
            &mut journal,
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            memory,
            Arc::new(Named {
                proposals: vec![proposal()],
                model: None,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
        )
        .unwrap();
        let mut entry = journal.state.entries[&1].clone();
        entry.evaluating = true;
        journal.commit_entry(entry).unwrap();
        crate::run_proposal_h(
            &mut archive,
            &epoch,
            &proposal(),
            &mut Stages {
                fail,
                selection: quality,
                confirmation: 0.3,
            },
            &mut Human {
                approve: false,
                calls: 0,
            },
        )
        .unwrap();
        let dataset = journal.proposal_dataset(1, &archive).unwrap().unwrap();
        let row = &dataset.rows[0];
        assert_eq!(row.label, expected);
        assert!(row.interrupted);
        assert!(row.reason.is_none());
        if index == 0 {
            assert!((row.selection.as_ref().unwrap().delta - 0.4).abs() < 1e-12);
            assert_eq!(row.stage, ProposalStage::Confirmation);
        }
        if index == 1 {
            assert_eq!(row.selection.as_ref().unwrap().eligible, Some(false));
            assert_eq!(row.stage, ProposalStage::Selection);
        }
        if index == 2 {
            assert!(row.selection.as_ref().unwrap().eligible.is_none());
        }
        if index == 3 {
            assert_eq!(row.stage, ProposalStage::Screen);
        }
        let public = serde_json::to_string(&dataset).unwrap();
        assert!(!public.contains("fresh"));
        assert!(!public.contains("Guess"));
    }
}
#[test]
fn unrelated_archives_and_corrupt_provenance_do_not_supply_labels() {
    let dir = tempfile::tempdir().unwrap();
    let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let memory = memory(&dir.path().join("m"));
    let path = dir.path().join("j");
    let mut journal = EpochJournal::open(&path).unwrap();
    prepare(
        &mut journal,
        &archive,
        &checkpoint,
        &dossier,
        &epoch,
        memory.clone(),
        Arc::new(Named {
            proposals: vec![proposal()],
            model: None,
            calls: Arc::new(AtomicUsize::new(0)),
        }),
    )
    .unwrap();
    let empty = Archive::open(dir.path().join("empty")).unwrap();
    assert!(journal.proposal_dataset(1, &empty).is_err());
    let (mut other, _, _, _) = fixtures(&dir.path().join("other"));
    evaluate(&mut journal, &mut archive, &epoch, &memory, false);
    assert!(journal.proposal_dataset(1, &other).is_err());
    let mut different_epoch = epoch.clone();
    different_epoch.policy.margin = 0.06;
    crate::run_proposal_h(
        &mut other,
        &different_epoch,
        &proposal(),
        &mut Host::default(),
        &mut Human {
            approve: false,
            calls: 0,
        },
    )
    .unwrap();
    assert!(journal.proposal_dataset(1, &other).is_err());
    let saved = journal.state.entries[&1].clone();
    let mut changed = saved.clone();
    changed.outcome.candidates[0].candidate_id = Some(changed.incumbent_id.clone());
    journal.commit_entry(changed).unwrap();
    assert!(journal.proposal_dataset(1, &archive).is_err());
    journal.commit_entry(saved).unwrap();
    let original = std::fs::read(&path).unwrap();
    drop(journal);
    for field in [
        "priors",
        "proposal_sources",
        "generation_seed",
        "model",
        "selection_binding",
        "outcome",
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
        let entry = &mut value["entries"]["1"];
        match field {
            "priors" => entry[field] = serde_json::json!([1.1]),
            "proposal_sources" => {
                entry[field][0]["occurrence"]["item_index"] = serde_json::json!(9);
            }
            "generation_seed" => entry[field] = serde_json::Value::Null,
            "model" => entry[field] = serde_json::json!({"provider":"secret text","model":"x"}),
            "selection_binding" => entry[field]["policy"]["margin"] = serde_json::json!(-1),
            _ => entry[field]["candidates"][0]["proposal_index"] = serde_json::json!(9),
        }
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(
            matches!(EpochJournal::open(&path), Err(Error::ArchiveCorrupt)),
            "{field}"
        );
    }
}
#[test]
fn legacy_journals_do_not_invent_models_seeds_occurrences_or_selection_labels() {
    let dir = tempfile::tempdir().unwrap();
    let (mut archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let memory = memory(&dir.path().join("m"));
    let path = dir.path().join("j");
    let mut journal = EpochJournal::open(&path).unwrap();
    prepare(
        &mut journal,
        &archive,
        &checkpoint,
        &dossier,
        &epoch,
        memory.clone(),
        Arc::new(Named {
            proposals: vec![proposal()],
            model: None,
            calls: Arc::new(AtomicUsize::new(0)),
        }),
    )
    .unwrap();
    evaluate(&mut journal, &mut archive, &epoch, &memory, false);
    drop(journal);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for field in [
        "model",
        "generation_seed",
        "proposal_sources",
        "priors",
        "selection_binding",
    ] {
        value["entries"]["1"].as_object_mut().unwrap().remove(field);
    }
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let journal = EpochJournal::open(path).unwrap();
    let dataset = journal.proposal_dataset(1, &archive).unwrap().unwrap();
    assert!(dataset.model.is_none());
    assert!(dataset.generation_seed.is_none());
    assert!(dataset.rows[0].occurrence.is_none());
    assert!(dataset.rows[0].selection.is_none());
    assert!(dataset.metrics(1).unwrap().brier.is_none());
}

#[tokio::test]
async fn cancelled_generation_retains_pending_provenance_and_marks_accounting_incomplete() {
    struct Pending(Arc<tokio::sync::Notify>);
    impl ModelPort for Pending {
        fn identity(&self) -> Option<ModelIdentity> {
            Some(ModelIdentity {
                provider: "offline".into(),
                model: "blocked-fixture".into(),
            })
        }
        fn generate(&self, _: crate::ModelRequest) -> crate::ModelFuture {
            self.0.notify_one();
            Box::pin(std::future::pending())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let path = dir.path().join("j");
    let mut journal = EpochJournal::open(&path).unwrap();
    let memory = memory(&dir.path().join("m"));
    let started = Arc::new(tokio::sync::Notify::new());
    let cap = budget();
    let mut pending = Box::pin(prepare_retreat_epoch(
        &mut journal,
        Arc::new(Pending(started.clone())),
        memory,
        inputs(
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            GenerationMode::Direct,
            &cap,
        ),
    ));
    tokio::select! {()=started.notified()=>{}, result=&mut pending=>panic!("blocked provider unexpectedly returned: {result:?}")}
    drop(pending);
    let dataset = journal.proposal_dataset(1, &archive).unwrap().unwrap();
    assert!(!dataset.generation_complete);
    assert_eq!(dataset.rows, Vec::<ProposalOutcomeRow>::new());
    assert_eq!(dataset.model.unwrap().model, "blocked-fixture");
    assert_eq!(
        dataset.generation.unwrap().calls[0].status,
        GenerationCallStatus::Pending
    );
    drop(journal);
    let journal = EpochJournal::open(path).unwrap();
    assert!(
        !journal
            .proposal_dataset(1, &archive)
            .unwrap()
            .unwrap()
            .generation_complete
    );
}
