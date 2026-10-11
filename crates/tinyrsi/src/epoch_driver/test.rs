//! Generation contract and restart-safe epoch execution.
#![expect(clippy::unwrap_used, reason = "assert deterministic epoch fixtures")]
use super::*;
use crate::{DeploymentCheckpoint, Dossier, ModelPort};
#[test]
fn normalized_request_is_frozen_before_any_generation() {
    let dossier = crate::Dossier::build(
        1,
        &crate::Observation::default(),
        &[],
        &[],
        crate::MemoryHealth::default(),
    );
    let request = normalized_retreat_request(
        "h",
        &dossier,
        &crate::Package::default(),
        &[],
        GenerationMode::Retreat,
        &budget(),
    )
    .unwrap();
    request.validate().unwrap();
    assert_eq!(
        request.output.schema["properties"]["prediction"]["properties"]["metric"]["const"],
        "normalized_h"
    );
    assert_eq!(request.config.variations, 4);
    assert_eq!(request.config.items_per_variation, 5);
    assert_eq!(request.config.scoring.select_top_k, Some(20));
    let without = normalized_retreat_request(
        "h",
        &dossier,
        &crate::Package::default(),
        &[],
        GenerationMode::RetreatWithoutMemory,
        &budget(),
    )
    .unwrap();
    assert!(without.input.memory_namespace.is_none());
}
fn budget() -> GenerationBudget {
    GenerationBudget {
        total_tokens: 500_000,
        call_tokens: 32_768,
        output_tokens: 2048,
        timeout_ms: 1000,
        variations: 4,
        generations: 2,
        items_per_variation: 5,
        select_top_k: 20,
        judge_raters: 1,
        wall_clock_secs: 120,
    }
}
use crate::{
    Budget, Epoch, EvalPolicy, FamilyAnchor, MemoryHealth, NodeStatus, Observation, Operation,
    Package, Patch, PatchClass, Prediction, Proposal, ScorePenalties, ScoringPolicySpec,
    TranscriptReference, VerifierOutcome,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn proposal() -> Proposal {
    Proposal {
        title: "verify".into(),
        patch: Patch {
            class: PatchClass::Context,
            operations: vec![Operation::Context {
                path: "context/main.md".into(),
                item: "method".into(),
                before: Some("Guess.\n".into()),
                after: Some("Compute.\n".into()),
            }],
        },
        mechanism: "run checker".into(),
        prediction: Prediction {
            metric: "normalized_h".into(),
            minimum_gain: 0.05,
            nodes: vec!["n".into()],
        },
        transcript_refs: vec![1],
        risk: "cost".into(),
        effort: "one edit".into(),
    }
}
fn fixtures(path: &std::path::Path) -> (Archive, DeploymentCheckpoint, Dossier, HEpoch) {
    use tinyhivemind_lang::lineage::{Direction, ObjectiveScore, Verdict, VersionMetadata};
    let mut archive = Archive::open(path).unwrap();
    let mut package = Package::default();
    package.constitution.permitted_edits = vec![PatchClass::Context];
    package.manifest.context = vec!["context/main.md".into()];
    package.documents.insert(
        "context/main.md".into(),
        "<!-- item:method -->\nGuess.\n".into(),
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
                        value: 0.0,
                        direction: Direction::Maximize,
                    },
                )]),
                verdict: Verdict::Accepted,
                memory_watermarks: BTreeMap::new(),
            },
        )
        .unwrap();
    let observation = Observation {
        transcript: vec![TranscriptReference {
            sequence: 1,
            seat_id: "seat".into(),
            episode_id: Some("ep".into()),
        }],
        episodes: vec![],
    };
    let dossier = Dossier::build(
        1,
        &observation,
        &[],
        &[VerifierOutcome {
            node_id: "n".into(),
            status: NodeStatus::Failed,
        }],
        MemoryHealth::default(),
    );
    let checkpoint = DeploymentCheckpoint {
        epoch: 1,
        observations: BTreeMap::from([(1, observation)]),
        deployed_id: Some(archive.head().unwrap().id.clone()),
        ..DeploymentCheckpoint::default()
    };
    let epoch = HEpoch {
        epoch: Epoch {
            id: 1,
            class: PatchClass::Context,
            transcript_refs: vec![1],
            failure_nodes: vec!["n".into()],
            budget: Budget { max_tokens: 100 },
            policy: EvalPolicy::default(),
        },
        policy: ScoringPolicySpec {
            anchors: vec![FamilyAnchor {
                family: "code".into(),
                baseline: 0.0,
                reference: 1.0,
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
    };
    (archive, checkpoint, dossier, epoch)
}
struct Script {
    calls: Arc<AtomicUsize>,
    fail: bool,
    invalid: bool,
}
impl ModelPort for Script {
    fn generate(&self, request: crate::ModelRequest) -> crate::ModelFuture {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let failed = self.fail;
        let invalid = self.invalid;
        let system = request.system();
        let value = if system.starts_with("You are a strict reviewer") {
            serde_json::json!({"assessment":"ranked","ranking":[1]})
        } else if system.starts_with("You are one of several") {
            serde_json::json!({"novelty":5,"usefulness":5,"boden":"exploratory"})
        } else if system.starts_with("You propose.") || system.starts_with("You attack.") {
            serde_json::json!("mechanism")
        } else if invalid {
            serde_json::json!({"items":[{},proposal()]})
        } else {
            serde_json::json!({"items":[proposal()]})
        };
        let content = if value.is_string() {
            value.as_str().unwrap().as_bytes().to_vec()
        } else {
            serde_json::to_vec(&value).unwrap()
        };
        Box::pin(async move {
            if failed {
                Err(Error::HostFailed)
            } else {
                Ok(crate::ModelReply {
                    content,
                    input_tokens: 10,
                    output_tokens: 10,
                })
            }
        })
    }
}
fn inputs<'a>(
    archive: &'a Archive,
    checkpoint: &'a DeploymentCheckpoint,
    dossier: &'a Dossier,
    epoch: &'a HEpoch,
    mode: GenerationMode,
    budget: &'a GenerationBudget,
) -> RetreatEpochInput<'a> {
    RetreatEpochInput {
        generation_seed: 42,
        orchestrator_context: "Find one grounded mechanism.",
        hive_id: "h",
        archive,
        checkpoint,
        dossier,
        epoch,
        mode,
        budget,
    }
}
#[tokio::test]
async fn direct_generation_reopens_without_repeating_model_call() {
    let dir = tempfile::tempdir().unwrap();
    let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let budget = budget();
    let calls = Arc::new(AtomicUsize::new(0));
    let port = Arc::new(Script {
        calls: calls.clone(),
        fail: false,
        invalid: false,
    });
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
    let first = prepare_retreat_epoch(
        &mut journal,
        port.clone(),
        memory.clone(),
        inputs(
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            GenerationMode::Direct,
            &budget,
        ),
    )
    .await
    .unwrap();
    assert_eq!(first.proposals, vec![proposal()]);
    assert_eq!(first.meter.attempted_calls, 1);
    drop(journal);
    let mut journal = EpochJournal::open(&path).unwrap();
    let second = prepare_retreat_epoch(
        &mut journal,
        port,
        memory,
        inputs(
            &archive,
            &checkpoint,
            &dossier,
            &epoch,
            GenerationMode::Direct,
            &budget,
        ),
    )
    .await
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
#[tokio::test]
async fn actual_engine_counts_invalid_attempt_and_retains_durable_discovery() {
    use tinybackrooms::Memory;
    let dir = tempfile::tempdir().unwrap();
    let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let mut budget = budget();
    budget.variations = 1;
    budget.items_per_variation = 1;
    budget.select_top_k = 1;
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
    let mut journal = EpochJournal::open(dir.path().join("j")).unwrap();
    let prepared = prepare_retreat_epoch(
        &mut journal,
        Arc::new(Script {
            calls,
            fail: false,
            invalid: true,
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
    )
    .await
    .unwrap();
    assert_eq!(prepared.proposals.len(), 1);
    assert_eq!(prepared.invalid_items, 1);
    assert_eq!(journal.generation_invalid_items(1), Some(1));
    let audit = journal.generation_receipts(1).unwrap().unwrap();
    assert_eq!(audit.calls.len(), 1);
    assert_eq!(
        audit.calls[0].items,
        vec![
            GenerationDisposition::SchemaRejected,
            GenerationDisposition::Decoded,
        ]
    );
    let path = journal.path.clone();
    drop(journal);
    let journal = EpochJournal::open(path).unwrap();
    assert_eq!(journal.generation_receipts(1).unwrap(), Some(audit));
    assert_eq!(
        memory
            .recall(memory.namespace(), "", 10)
            .await
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn failed_generation_is_charged_and_never_retried_on_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (archive, checkpoint, dossier, epoch) = fixtures(&dir.path().join("a"));
    let budget = budget();
    let calls = Arc::new(AtomicUsize::new(0));
    let port = Arc::new(Script {
        calls: calls.clone(),
        fail: true,
        invalid: false,
    });
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
    assert!(
        prepare_retreat_epoch(
            &mut journal,
            port.clone(),
            memory.clone(),
            inputs(
                &archive,
                &checkpoint,
                &dossier,
                &epoch,
                GenerationMode::Direct,
                &budget
            )
        )
        .await
        .is_err()
    );
    assert_eq!(journal.generation_meter(1).unwrap().failed_calls, 1);
    drop(journal);
    let mut journal = EpochJournal::open(&path).unwrap();
    assert!(
        prepare_retreat_epoch(
            &mut journal,
            port,
            memory,
            inputs(
                &archive,
                &checkpoint,
                &dossier,
                &epoch,
                GenerationMode::Direct,
                &budget
            )
        )
        .await
        .is_err()
    );
    let audit = journal.generation_receipts(1).unwrap().unwrap();
    assert_eq!(audit.calls[0].status, GenerationCallStatus::Failed);
    assert_eq!(audit.calls[0].items, Vec::<GenerationDisposition>::new());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
mod evaluation;

mod failures;

mod receipts;
