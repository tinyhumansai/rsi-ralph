//! Separate bounded validation of a model-authored repair and real deployment.
use super::{driver, fixtures};
use std::{collections::BTreeMap, path::Path, sync::Arc};
use rsi_ralph_host::{
    Archive, Budget, ControlledRunnerFactory, CoordinatorEvaluator, CoordinatorHost,
    DeploymentStore, GenerationMode, HostVerifier, ModelPort, ModelRequest, PrivateTask, Proposal,
    ReasonCode, SnapshotMemoryStore, normalized_retreat_request, run_proposal_h,
};

pub(super) fn run(
    root: &Path,
    mut archive: Archive,
    model: &Arc<dyn ModelPort>,
    memory: &SnapshotMemoryStore,
    evaluator: &mut CoordinatorEvaluator,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let baseline = archive.head().ok_or("missing baseline")?.id.clone();
    let (_, dossier) = driver::observed(root, &archive, model.clone(), memory.clone())?;
    let epoch = driver::epoch(&dossier, evaluator);
    let schema = constrained_schema(&dossier)?;
    let request = ModelRequest::new(
        "Propose exactly the permitted context repair. Return native tool arguments. Private tasks, outputs and graders are unavailable.".into(),
        format!("{}\nPublic context item before: {:?}\nPublic dossier: {}", driver::EVOLVED, fixtures::BEFORE, serde_json::to_string(&dossier)?),
        8001, 131_072, 2048, 120_000,
    )?.with_tool("submit_proposal".into(), schema)?;
    let reply = driver::runtime()?.block_on(model.generate(request))?;
    let proposal: Proposal = serde_json::from_slice(&reply.content)?;
    let denied = run_proposal_h(
        &mut archive,
        &epoch,
        &proposal,
        evaluator,
        &mut fixtures::Deny,
    )?;
    if denied.reason != ReasonCode::ApprovalDenied
        || archive.head().is_none_or(|h| h.id != baseline)
    {
        return Err("model repair did not pass confirmation and preserve denied baseline".into());
    }
    let mut approved_archive = Archive::open(root.join("approved-archive.json"))?;
    fixtures::founding(&mut approved_archive)?;
    let accepted = run_proposal_h(
        &mut approved_archive,
        &epoch,
        &proposal,
        evaluator,
        &mut fixtures::ContextGate,
    )?;
    if accepted.reason != ReasonCode::Accepted {
        return Err("authorized isolated context repair was not accepted".into());
    }
    let deployed = deploy_and_rollback(root, &mut approved_archive, model, memory)?;
    if !deployed {
        return Err("reopened deployment lost rollback identity".into());
    }
    drop(approved_archive);
    let approved_archive = Archive::open(root.join("approved-archive.json"))?;
    if approved_archive.head().is_none_or(|h| h.id != baseline) {
        return Err("reopened archive lost parent rollback".into());
    }
    std::fs::write(
        root.join("controlled-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "source":"live", "purpose":"isolated integration validation, outside preregistered comparison",
            "generation_tokens":reply.input_tokens + reply.output_tokens,
            "denied":format!("{:?}", denied.reason), "accepted":format!("{:?}",accepted.reason),
            "comparisons":archive.normalized_evaluations().iter().map(|e|serde_json::json!({"stage":e.stage,"comparison":e.comparison})).collect::<Vec<_>>(),
            "deployment_and_parent_rollback":deployed,
            "approval":"user-authorized context-only sandbox; not a production human approval service"
        }))?,
    )?;
    println!(
        "PASS: model-authored normalized repair, all three private stages, deny/accept, SQLite activation and exact parent rollback"
    );
    Ok(())
}
fn constrained_schema(dossier: &rsi_ralph_host::Dossier) -> rsi_ralph_host::Result<serde_json::Value> {
    let mut schema = normalized_retreat_request(
        "controlled",
        dossier,
        &fixtures::package(),
        &[],
        GenerationMode::Direct,
        &driver::generation(1, 500_000),
    )?
    .output
    .schema;
    schema["properties"]["transcript_refs"] =
        serde_json::json!({"const":dossier.transcript_sequences});
    schema["properties"]["prediction"]["properties"]["nodes"] =
        serde_json::json!({"const":dossier.failed_nodes});
    let properties = &mut schema["properties"]["patch"]["properties"];
    properties["class"] = serde_json::json!({"const":"context"});
    properties["operations"] = serde_json::json!({"type":"array","minItems":1,"maxItems":1,"items":{"type":"object","additionalProperties":false,"required":["operation","path","item","before","after"],"properties":{"operation":{"const":"context"},"path":{"const":"context/method.md"},"item":{"const":"method"},"before":{"const":fixtures::BEFORE},"after":{"type":"string","minLength":1,"maxLength":512,"pattern":"\n$"}}}});
    Ok(schema)
}
fn deploy_and_rollback(
    root: &Path,
    archive: &mut Archive,
    model: &Arc<dyn ModelPort>,
    memory: &SnapshotMemoryStore,
) -> std::result::Result<bool, Box<dyn std::error::Error>> {
    let factory = Arc::new(ControlledRunnerFactory::new(
        model.clone(),
        memory.clone(),
        PrivateTask::new(
            "activation-probe".into(),
            "harness".into(),
            "solver".into(),
            "Return exactly READY".into(),
            HostVerifier::Golden {
                expected: b"READY".to_vec(),
            },
            vec![1],
        )?,
        Budget { max_tokens: 16_384 },
        256,
        120_000,
    )?);
    let mut host = CoordinatorHost::sqlite(
        "activation".into(),
        root.join("activation.sqlite"),
        factory.clone(),
        Arc::new(memory.clone()),
    )?;
    let mut store = DeploymentStore::open(root.join("activation.json"))?;
    let rt = driver::runtime()?;
    rt.block_on(store.activate(archive, &mut host))?;
    if store.checkpoint().deployed_id.as_deref() != archive.head().map(|h| h.id.as_str()) {
        return Err("activation did not select accepted candidate".into());
    }
    verify_live_turn(&rt, &host, &factory)?;
    rt.block_on(store.rollback(archive, &mut host))?;
    let restored = archive.head().ok_or("rollback lost head")?;
    if archive.package(&restored.id) != Some(&fixtures::package())
        || !memory.active_watermarks()?.is_empty()
    {
        return Err("rollback lost exact parent package or memory".into());
    }
    // Reopen durable deployment and SQLite runtime, then reattach the exact parent.
    drop(store);
    drop(host);
    let mut store = DeploymentStore::open(root.join("activation.json"))?;
    let mut host = CoordinatorHost::sqlite(
        "activation-reopened".into(),
        root.join("activation.sqlite"),
        factory,
        Arc::new(memory.clone()),
    )?;
    rt.block_on(store.recover(archive, &mut host))?;
    if host.coordinator().is_none() {
        return Err("SQLite parent runtime did not reattach".into());
    }
    Ok(
        store.checkpoint().deployed_id.as_deref() == Some(restored.id.as_str())
            && restored.memory_watermarks == BTreeMap::new(),
    )
}

fn verify_live_turn(
    rt: &tokio::runtime::Runtime,
    host: &CoordinatorHost,
    factory: &ControlledRunnerFactory,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let coordinator = host.coordinator().ok_or("missing accepted runtime")?;
    rt.block_on(coordinator.send_as_host(tinyhivemind_hives::SendMessage {
        message_id: "accepted-probe".into(),
        sender: "host".into(),
        thread: None,
        only_for: vec![],
        starters: vec![],
        destination: tinyhivemind_hives::Destination::Hive("work".into()),
        body: "Public activation health probe: return exactly READY".into(),
    }))?;
    rt.block_on(coordinator.run_until_idle())?;
    let evidence = factory.evidence()?;
    if evidence.len() != 1 || !evidence[0].passed {
        return Err("deployed runner failed host verification".into());
    }
    Ok(())
}
