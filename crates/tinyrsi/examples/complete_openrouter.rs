//! Opt-in full local integration and preregistered `OpenRouter` experiment.
#[path = "complete/controlled.rs"]
mod controlled;
#[path = "complete/driver.rs"]
mod driver;
#[path = "complete/fixtures.rs"]
mod fixtures;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tinyrsi::{
    Archive, Budget, BudgetedModel, CoordinatorEvaluator, DurableRetreatMemory, EpochJournal,
    EvidenceSource, ExperimentProtocol, ExperimentProtocolSpec, GenerationMode, MemoryLimits,
    ModelPort, OpenRouterModel, RetreatEpochInput, SnapshotMemoryStore, SuiteFingerprints,
    TrialSeeds, evaluate_retreat_epoch, prepare_retreat_epoch, run_experiment,
};
fn fingerprint(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    if std::env::var("TINYRSI_LIVE_OPENROUTER").as_deref() != Ok("1") {
        println!(
            "Set TINYRSI_LIVE_OPENROUTER=1 to run the bounded local experiment using inherited OPENROUTER_API_KEY."
        );
        return Ok(());
    }
    let root = std::env::var("TINYRSI_COMPLETE_DIR")
        .map_or_else(|_| PathBuf::from("target/rsi-complete-live"), PathBuf::from);
    if root.exists() {
        return Err("choose a fresh TINYRSI_COMPLETE_DIR to preserve preregistered trials".into());
    }
    std::fs::create_dir_all(&root)?;
    let lean = std::env::var("TINYRSI_LEAN")?;
    let model_id =
        std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "openai/gpt-4.1-mini".into());
    let model: Arc<dyn ModelPort> = Arc::new(OpenRouterModel::new(model_id.clone())?);
    let suites = fixtures::suites(&root, &lean)?;
    let suite_ids = suites.fingerprints();
    let mut archive = Archive::open(root.join("worked-archive.json"))?;
    let baseline = fixtures::founding(&mut archive)?;
    let memory = SnapshotMemoryStore::open(
        root.join("hive-memory.json"),
        vec!["TINYRSI_PRIVATE_CANARY_2026".into()],
        1_000_000,
    )?;
    memory.seed(&fixtures::package(), &BTreeMap::new(), BTreeMap::new())?;
    let evaluation_model = Arc::new(BudgetedModel::new(model.clone(), 5_000_000)?);
    let mut evaluator = CoordinatorEvaluator::new(
        suites.clone(),
        evaluation_model.clone(),
        memory.clone(),
        256,
        120_000,
    )?;
    if std::env::var("TINYRSI_CONTROLLED_ONLY").as_deref() == Ok("1") {
        controlled::run(&root, archive, &model, &memory, &mut evaluator)?;
        return Ok(());
    }
    // Preregister configurations, budgets and independent draws BEFORE generation.
    let protocol = ExperimentProtocol::new(ExperimentProtocolSpec {
        original_config_id: driver::config_id(driver::ORIGINAL),
        evolved_config_id: driver::config_id(driver::EVOLVED),
        initial_hive_id: baseline.clone(),
        suites: SuiteFingerprints {
            selection: suite_ids["selection"].clone(),
            confirmation: suite_ids["confirmation"].clone(),
            anchor: suite_ids["anchor"].clone(),
        },
        trials: [(1001, 101), (2001, 201), (3001, 301), (4001, 401)]
            .into_iter()
            .map(|(proposal_seed, seed)| TrialSeeds {
                proposal_seed,
                evaluation_seeds: vec![seed, seed + 1],
            })
            .collect(),
        attempts_per_trial: 1,
        generation_cap: 500_000,
        evaluation_cap: 500_000,
        per_run_budget: Budget { max_tokens: 16_384 },
        scoring: fixtures::policy(),
        efficiency_margin: 0.01,
        source: EvidenceSource::Live,
    })?;
    std::fs::write(
        root.join("preregistration.json"),
        serde_json::to_vec_pretty(protocol.spec())?,
    )?;
    write_provenance(&root, &model_id, &protocol)?;
    worked(
        &root,
        archive,
        &baseline,
        &model,
        &memory,
        &mut evaluator,
        &model_id,
    )?;
    let mut trials = driver::Trials {
        root: root.clone(),
        model: model.clone(),
        suites: suites.clone(),
        index: 0,
    };
    let report = run_experiment(&protocol, &mut trials)?;
    std::fs::write(
        root.join("experiment-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "Experiment: {} live attempts; {:?}; reserved tokens generation={} evaluation={}",
        report.attempts, report.verdict, report.generation_tokens, report.evaluation_tokens
    );
    println!("Evidence: {}", root.display());
    Ok(())
}

fn write_provenance(
    root: &std::path::Path,
    model_id: &str,
    protocol: &ExperimentProtocol,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::fs::write(
        root.join("configuration-provenance.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "origin": "two human-authored frozen proposer configurations; evolved is a protocol arm name, not a learned configuration",
            "original": driver::ORIGINAL,
            "evolved": driver::EVOLVED,
            "public_prior_history": driver::HISTORY,
            "calibration": "fixed theoretical family floor zero and ceiling one; no empirical calibration claim",
            "model": model_id,
            "protocol_fingerprint": protocol.fingerprint()
        }))?,
    )?;
    Ok(())
}

fn worked(
    root: &std::path::Path,
    mut archive: Archive,
    baseline: &str,
    model: &Arc<dyn ModelPort>,
    memory: &SnapshotMemoryStore,
    evaluator: &mut CoordinatorEvaluator,
    model_id: &str,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let (store, dossier) = driver::observed(root, &archive, model.clone(), memory.clone())?;
    let epoch = driver::epoch(&dossier, evaluator);
    let discovery = Arc::new(DurableRetreatMemory::open(
        root.join("retreat-memory.json"),
        "worked",
        MemoryLimits {
            max_entries: 1000,
            max_bytes: 4_000_000,
        },
        vec!["TINYRSI_PRIVATE_CANARY_2026".into()],
    )?);
    let mut journal = EpochJournal::open(root.join("worked-epoch.json"))?;
    let budget = driver::generation(25, 3_000_000);
    let prepared = driver::runtime()?.block_on(prepare_retreat_epoch(
        &mut journal,
        model.clone(),
        discovery.clone(),
        RetreatEpochInput {
            hive_id: "worked",
            archive: &archive,
            checkpoint: store.checkpoint(),
            dossier: &dossier,
            epoch: &epoch,
            mode: GenerationMode::Retreat,
            budget: &budget,
            orchestrator_context: driver::EVOLVED,
            generation_seed: 7001,
        },
    ))?;
    println!(
        "Worked retreat: {} returned proposals, {} invalid attempts, {} calls, {} measured tokens",
        prepared.proposals.len(),
        prepared.invalid_items,
        prepared.meter.attempted_calls,
        prepared.meter.input_tokens + prepared.meter.output_tokens
    );
    if prepared.proposals.len() < 20 {
        return Err(
            "worked retreat returned fewer than 20 typed proposals; evidence retained".into(),
        );
    }
    let outcome = evaluate_retreat_epoch(
        &mut journal,
        &mut archive,
        &epoch,
        evaluator,
        &mut fixtures::Deny,
        &discovery,
    )?;
    if outcome.candidates.len() < 20 || archive.head().is_none_or(|h| h.id != baseline) {
        return Err("worked denial funnel did not retain baseline and all attempts".into());
    }
    let comparisons = archive
        .normalized_evaluations()
        .iter()
        .map(|event| serde_json::json!({"stage":event.stage,"comparison":event.comparison}))
        .collect::<Vec<_>>();
    std::fs::write(
        root.join("worked-report.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"source":"live","model":model_id,"baseline":"deliberately impaired integration calibration, not representative benchmark","generated":prepared.proposals.len(),"invalid":prepared.invalid_items,"meter":prepared.meter,"outcomes":outcome,"comparisons":comparisons,"approval":if outcome.candidates.iter().any(|candidate|candidate.reason==tinyrsi::ReasonCode::ApprovalDenied){"denied; baseline preserved"}else{"not reached; baseline preserved"}}),
        )?,
    )?;
    drop(journal);
    drop(archive);
    let archive = Archive::open(root.join("worked-archive.json"))?;
    if archive.head().is_none_or(|h| h.id != baseline) {
        return Err("restart changed denied deployment".into());
    }
    Ok(())
}
