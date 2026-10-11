//! Public-schema generation with hard model reservations and durable intent.
use super::{EpochJournal, GenerationBudget, GenerationMode, PreparedRetreatEpoch, types::Entry};
use crate::{
    Archive, DeploymentCheckpoint, Dossier, DurableRetreatMemory, Error, HEpoch, ModelConversant,
    ModelPort, Package, Proposal, ProposalEvent, Result,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tinybackrooms::{Conversant, Engine, GenerateRequest};
/// Frozen observed epoch inputs; the archive and checkpoint remain host-owned.
pub struct RetreatEpochInput<'a> {
    /// Frozen generation seed, independent of the observed epoch identity.
    pub generation_seed: u64,
    /// Frozen public orchestrator instructions applied to actual model input.
    pub orchestrator_context: &'a str,
    /// Actual runtime hive identity.
    pub hive_id: &'a str,
    /// Durable selected package archive.
    pub archive: &'a Archive,
    /// Persisted observation checkpoint.
    pub checkpoint: &'a DeploymentCheckpoint,
    /// Redacted dossier built from that observed epoch.
    pub dossier: &'a Dossier,
    /// Immutable normalized scoring and private suites.
    pub epoch: &'a HEpoch,
    /// Experimental generation arm.
    pub mode: GenerationMode,
    /// Shared hard model caps and creative run shape.
    pub budget: &'a GenerationBudget,
}
/// Builds a real Engine contract that requests `normalized_h` before generation.
/// # Errors
/// Rejects invalid public input, generation bounds or contract settings.
pub fn normalized_retreat_request(
    hive_id: &str,
    dossier: &Dossier,
    incumbent: &Package,
    rejected: &[ProposalEvent],
    mode: GenerationMode,
    budget: &GenerationBudget,
) -> Result<GenerateRequest> {
    if budget.total_tokens == 0
        || budget.call_tokens == 0
        || budget.call_tokens > budget.total_tokens
        || budget.output_tokens == 0
        || budget.timeout_ms == 0
        || budget.timeout_ms > 300_000
        || budget.select_top_k == 0
        || budget.wall_clock_secs == 0
    {
        return Err(Error::InvalidProposal);
    }
    let mut request = crate::typed_retreat_request(hive_id, dossier, incumbent, rejected)?;
    request.output.schema["properties"]["prediction"]["properties"]["metric"]["const"] =
        json!("normalized_h");
    request.input.brief.push_str(" Predict normalized_h gain under the host's frozen verified quality, health and cost policy. Output prediction.metric exactly normalized_h.");
    for rule in &mut request.input.constraints.must {
        *rule = rule.replace("verified_quality", "normalized_h");
    }
    request.config.variations = budget.variations;
    request.config.generations = budget.generations;
    request.config.items_per_variation = budget.items_per_variation;
    request.config.scoring.select_top_k =
        Some(usize::try_from(budget.select_top_k).map_err(|_| Error::InvalidProposal)?);
    request.config.scoring.judge_raters = budget.judge_raters;
    request.config.budget.max_turns = Some(budget.generations);
    request.config.budget.wall_clock_secs = Some(budget.wall_clock_secs);
    if mode != GenerationMode::Retreat {
        request.input.memory_namespace = None;
    }
    request.validate().map_err(|_| Error::InvalidProposal)?;
    Ok(request)
}
/// Prepares one observed epoch once, using actual Engine or direct native-tool generation.
///
/// A durable intent precedes model execution. Completed generation reopens without
/// model calls. An interrupted unknown call stays blocked; it is never retried
/// implicitly. Invalid artifacts and failed attempts retain safe occurrence receipts, not raw text.
/// # Errors
/// Returns stale observation/configuration, interrupted generation or host errors.
pub async fn prepare_retreat_epoch(
    journal: &mut EpochJournal,
    port: Arc<dyn ModelPort>,
    memory: Arc<DurableRetreatMemory>,
    input: RetreatEpochInput<'_>,
) -> Result<PreparedRetreatEpoch> {
    validate_input(&input, &memory)?;
    let previous = journal.state.entries.get(&input.epoch.epoch.id).cloned();
    let incumbent_id = previous
        .as_ref()
        .map(|e| e.incumbent_id.clone())
        .or_else(|| input.archive.head().map(|h| h.id.clone()))
        .ok_or(Error::InvalidRecord)?;
    let package = input
        .archive
        .package(&incumbent_id)
        .ok_or(Error::InvalidRecord)?;
    let fingerprint = fingerprint(&input, &incumbent_id)?;
    if let Some(previous) = previous {
        journal.generation_receipts(input.epoch.epoch.id)?;
        if previous.fingerprint != fingerprint {
            return Err(Error::InvalidProposal);
        }
        let prepared = previous.prepared.ok_or(Error::HostFailed)?;
        validate_prepared(&prepared, &memory)?;
        return Ok(prepared);
    }
    if input.checkpoint.pending.is_some()
        || input.checkpoint.deployed_id.as_deref() != Some(incumbent_id.as_str())
    {
        return Err(Error::InvalidRecord);
    }
    let rejected: Vec<_> = input.archive.proposals().into_iter().cloned().collect();
    let mut request = normalized_retreat_request(
        input.hive_id,
        input.dossier,
        package,
        &rejected,
        input.mode,
        input.budget,
    )?;
    request
        .input
        .brief
        .push_str("\n\nFrozen orchestrator instructions:\n");
    request.input.brief.push_str(input.orchestrator_context);
    request.config.seed = input.generation_seed;
    if input.mode == GenerationMode::Retreat
        && request.input.memory_namespace.as_deref() != Some(memory.namespace())
    {
        return Err(Error::InvalidProposal);
    }
    let backend = Arc::new(ModelConversant::new(
        port,
        input.generation_seed,
        input.budget.total_tokens,
        input.budget.call_tokens,
        input.budget.output_tokens,
        input.budget.timeout_ms,
    )?);
    let mut entry = Entry::intent(
        input.epoch.epoch.id,
        incumbent_id,
        fingerprint,
        backend.meter()?,
    );
    journal.commit_entry(entry.clone())?;
    let receipts = Arc::new(super::receipts::ReceiptStore::create(
        &journal.path,
        entry.epoch,
        entry.fingerprint.clone(),
        input.mode,
        journal.writer_lock.clone(),
    )?);
    let audited = Arc::new(AuditedConversant {
        receipts: receipts.clone(),
        memory: memory.clone(),
        backend: backend.clone(),
        schema: request.output.schema.clone(),
        invalid: AtomicUsize::new(0),
        attempts: Mutex::new(Vec::new()),
    });
    let response = generate(
        audited.clone(),
        memory.clone(),
        &request,
        input.mode,
        input.budget.select_top_k,
    )
    .await;
    entry.meter = backend.meter()?;
    entry.invalid_items = audited.invalid.load(Ordering::Relaxed);
    let response = response.and_then(|values| receipts.settled().map(|()| values));
    let values = match response {
        Ok(values) => values,
        Err(error) => {
            entry.generation_failed = true;
            journal.commit_entry(entry)?;
            return Err(error);
        }
    };
    finish_preparation(journal, entry, values, &memory, &input)
}
fn validate_prepared(prepared: &PreparedRetreatEpoch, memory: &DurableRetreatMemory) -> Result<()> {
    for proposal in &prepared.proposals {
        memory.validate_public_text(
            &serde_json::to_string(proposal).map_err(|_| Error::InvalidProposal)?,
        )?;
    }
    Ok(())
}
fn finish_preparation(
    journal: &mut EpochJournal,
    mut entry: Entry,
    generated: Generated,
    memory: &DurableRetreatMemory,
    input: &RetreatEpochInput<'_>,
) -> Result<PreparedRetreatEpoch> {
    let mut proposals = Vec::new();
    let mut invalid_items = entry.invalid_items;
    let mut ranked_items = 0;
    for (index, value) in generated.values.into_iter().enumerate() {
        let decoded = serde_json::from_value::<Proposal>(value.clone());
        if let Ok(proposal) = decoded
            && proposal.prediction.metric == "normalized_h"
            && memory.validate_public_text(&value.to_string()).is_ok()
        {
            ranked_items += usize::from(index < generated.ranked_items);
            proposals.push(proposal);
        } else {
            invalid_items = invalid_items.saturating_add(1);
        }
    }
    let prepared = PreparedRetreatEpoch {
        epoch: entry.epoch,
        incumbent_id: entry.incumbent_id.clone(),
        mode: input.mode,
        fingerprint: entry.fingerprint.clone(),
        policy_fingerprint: policy_fingerprint(input.epoch)?,
        proposals,
        ranked_items,
        invalid_items,
        meter: entry.meter.clone(),
    };
    entry.invalid_items = prepared.invalid_items;
    entry.prepared = Some(prepared.clone());
    journal.commit_entry(entry)?;
    Ok(prepared)
}
struct Generated {
    values: Vec<Value>,
    ranked_items: usize,
}

async fn generate(
    backend: Arc<AuditedConversant>,
    memory: Arc<DurableRetreatMemory>,
    request: &GenerateRequest,
    mode: GenerationMode,
    count: u32,
) -> Result<Generated> {
    if mode == GenerationMode::Direct {
        let schema = json!({"type":"object","required":["items"],"additionalProperties":false,"properties":{"items":{"type":"array","items":request.output.schema,"maxItems":count}}});
        let response=backend.structured(tinybackrooms::StructuredRequest{seat:tinybackrooms::Seat{run_id:"direct".into(),role:"proposer".into()},model:None,system:"Propose guarded hive edits directly. Predict normalized_h, follow the supplied schema, and ground all references in the public dossier. Model opinions never establish verification or approval.".into(),messages:vec![tinybackrooms::Message::user(serde_json::to_string(&request.input).map_err(|_|Error::InvalidProposal)?)],tool:"submit_items".into(),schema:schema.clone()}).await.map_err(|_|Error::HostFailed)?;
        let validator = jsonschema::validator_for(&schema).map_err(|_| Error::InvalidProposal)?;
        if !validator.is_valid(&response.value) {
            return Err(Error::InvalidProposal);
        }
        let values = response.value["items"]
            .as_array()
            .cloned()
            .ok_or(Error::InvalidProposal)?;
        return Ok(Generated {
            ranked_items: values.len(),
            values,
        });
    }
    let engine = Engine::new(backend.clone()).with_memory(memory);
    let response = engine
        .generate(request)
        .await
        .map_err(|_| Error::HostFailed)?;
    if response.warnings.iter().any(|warning| {
        warning.starts_with("memory recall skipped:")
            || warning.starts_with("memory store skipped:")
    }) {
        return Err(Error::HostFailed);
    }
    let mut values: Vec<_> = response.items.into_iter().map(|item| item.value).collect();
    let ranked_items = values.len();
    let mut remaining = values.clone();
    for attempt in backend
        .attempts
        .lock()
        .map_err(|_| Error::HostFailed)?
        .iter()
    {
        if let Some(index) = remaining.iter().position(|selected| selected == attempt) {
            remaining.remove(index);
        } else {
            values.push(attempt.clone());
        }
    }
    Ok(Generated {
        values,
        ranked_items,
    })
}
fn validate_observation(input: &RetreatEpochInput<'_>) -> Result<()> {
    let id = input.epoch.epoch.id;
    let observed = input
        .checkpoint
        .observations
        .get(&id)
        .ok_or(Error::InvalidProposal)?;
    let refs: Vec<_> = observed.transcript.iter().map(|r| r.sequence).collect();
    if input.dossier.epoch != id
        || input.checkpoint.epoch < id
        || input.dossier.transcript_sequences != refs
        || input.epoch.epoch.transcript_refs != refs
    {
        return Err(Error::InvalidProposal);
    }
    let mut nodes = input.dossier.failed_nodes.clone();
    nodes.extend(input.dossier.open_nodes.iter().cloned());
    nodes.sort();
    nodes.dedup();
    let mut expected = input.epoch.epoch.failure_nodes.clone();
    expected.sort();
    expected.dedup();
    if nodes != expected {
        return Err(Error::InvalidProposal);
    }
    Ok(())
}
fn validate_input(input: &RetreatEpochInput<'_>, memory: &DurableRetreatMemory) -> Result<()> {
    validate_observation(input)?;
    if input.orchestrator_context.trim().is_empty() {
        return Err(Error::InvalidProposal);
    }
    memory.validate_public_text(input.orchestrator_context)
}
fn fingerprint(input: &RetreatEpochInput<'_>, incumbent: &str) -> Result<String> {
    hash(
        &json!({"incumbent":incumbent,"hive_id":input.hive_id,"generation_seed":input.generation_seed,"orchestrator_context":input.orchestrator_context,"dossier":input.dossier,"mode":input.mode,"generation_budget":input.budget,"policy":policy_fingerprint(input.epoch)?}),
    )
}
pub(super) fn policy_fingerprint(epoch: &HEpoch) -> Result<String> {
    hash(
        &json!({"id":epoch.epoch.id,"class":epoch.epoch.class,"refs":epoch.epoch.transcript_refs,"nodes":epoch.epoch.failure_nodes,"budget":epoch.epoch.budget,"policy":epoch.policy,"suites":epoch.suite_ids}),
    )
}
fn hash(value: &Value) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(|_| Error::InvalidProposal)?)
    ))
}
struct AuditedConversant {
    backend: Arc<ModelConversant>,
    schema: Value,
    invalid: AtomicUsize,
    attempts: Mutex<Vec<Value>>,
    receipts: Arc<super::receipts::ReceiptStore>,
    memory: Arc<DurableRetreatMemory>,
}
impl Conversant for AuditedConversant {
    fn turn(
        &self,
        request: tinybackrooms::TurnRequest,
    ) -> tinybackrooms::BoxFuture<'_, tinybackrooms::Result<tinybackrooms::TurnReply>> {
        self.backend.turn(request)
    }
    fn structured(
        &self,
        request: tinybackrooms::StructuredRequest,
    ) -> tinybackrooms::BoxFuture<'_, tinybackrooms::Result<tinybackrooms::StructuredReply>> {
        Box::pin(async move {
            let proposals = request.tool == "submit_items";
            let call = if proposals {
                Some(self.receipts.begin().map_err(audit_error)?)
            } else {
                None
            };
            let response = match self.backend.structured(request).await {
                Ok(response) => response,
                Err(error) => {
                    if let Some(call) = call {
                        self.receipts
                            .finish(call, super::GenerationCallStatus::Failed, vec![])
                            .map_err(audit_error)?;
                    }
                    return Err(error);
                }
            };
            if proposals {
                let validator = jsonschema::validator_for(&self.schema).map_err(|_| {
                    tinybackrooms::Error::Conversant("invalid frozen proposal schema".into())
                })?;
                if let Some(call) = call {
                    let (status, items) = response
                        .value
                        .get("items")
                        .and_then(Value::as_array)
                        .map_or((super::GenerationCallStatus::Malformed, vec![]), |items| {
                            (
                                super::GenerationCallStatus::Returned,
                                items
                                    .iter()
                                    .map(|item| {
                                        if !validator.is_valid(item) {
                                            super::GenerationDisposition::SchemaRejected
                                        } else if serde_json::from_value::<Proposal>(item.clone())
                                            .is_err()
                                        {
                                            super::GenerationDisposition::DecodeRejected
                                        } else if self
                                            .memory
                                            .validate_public_text(&item.to_string())
                                            .is_err()
                                        {
                                            super::GenerationDisposition::PrivacyRejected
                                        } else {
                                            super::GenerationDisposition::Decoded
                                        }
                                    })
                                    .collect(),
                            )
                        });
                    self.receipts
                        .finish(call, status, items)
                        .map_err(audit_error)?;
                }
                let rejected = response
                    .value
                    .get("items")
                    .and_then(Value::as_array)
                    .map_or(1, |items| {
                        items
                            .iter()
                            .filter(|item| !validator.is_valid(item))
                            .count()
                    });
                self.invalid.fetch_add(rejected, Ordering::Relaxed);
                if let Some(items) = response.value.get("items").and_then(Value::as_array) {
                    self.attempts
                        .lock()
                        .map_err(|_| {
                            tinybackrooms::Error::Conversant("proposal audit unavailable".into())
                        })?
                        .extend(
                            items
                                .iter()
                                .filter(|item| validator.is_valid(item))
                                .cloned(),
                        );
                }
            }
            Ok(response)
        })
    }
}
fn audit_error(_: Error) -> tinybackrooms::Error {
    tinybackrooms::Error::Conversant("proposal audit unavailable".into())
}
impl std::fmt::Debug for RetreatEpochInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetreatEpochInput")
            .field("epoch", &self.epoch.epoch.id)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}
