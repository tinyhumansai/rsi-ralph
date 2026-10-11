//! Creative retreat using the tinybackrooms HTTP wire contract.
//!
//! Only a redacted public package projection is sent to proposal models. Model
//! rankings are proposal discovery signals; host evaluation and approval remain
//! separate. The caller owns HTTP transport and the service's memory adapter.
mod evolver;
mod types;
use crate::{Error, Result};
pub use evolver::{RetreatBrief, RetreatEvolver};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tinyrsi_types::{Package, Proposal, ProposalEvent};
pub use types::{RetreatDossier, RetreatOutput, RetreatTransport};

/// Builds a JSON request for tinybackrooms `POST /v1/generate`.
///
/// The prompt includes public manifest structure, referenced public Markdown,
/// redacted dossier counters and rejection metadata. It excludes constitution,
/// seat factory configuration, goal acceptance criteria and raw rejected edits.
/// Only namespace routing is requested; persistence is owned by tinybackrooms.
///
/// # Errors
/// Returns [`Error::InvalidProposal`] for an empty or unsafe hive identifier,
/// or a malformed canonical JSON frontmatter header.
pub fn build_retreat_request(
    hive_id: &str,
    dossier: &impl RetreatDossier,
    incumbent: &Package,
    rejected: &[ProposalEvent],
) -> Result<Value> {
    if hive_id.is_empty()
        || !hive_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(Error::InvalidProposal);
    }
    let secrets = protected_strings(incumbent)?;
    if secrets.iter().any(|secret| hive_id.contains(secret)) {
        return Err(Error::InvalidProposal);
    }
    let mut public = public_package(incumbent, &secrets);
    redact(&mut public, &secrets);
    let mut evidence = serde_json::to_value(dossier).map_err(|_| Error::InvalidProposal)?;
    redact(&mut evidence, &secrets);
    let prior: Vec<_> = rejected
        .iter()
        .map(|event| {
            json!({
                "epoch": event.epoch,
                "class": event.patch.class,
                "reason": public_reason(&event.reason),
                "operation_count": event.patch.operations.len(),
            })
        })
        .collect();
    let request = json!({
        "input": {
            "brief": "Propose one guarded hive edit per item to address the public failure dossier. Use concrete mechanisms, testable quality-gain predictions, evidence references, risks and effort. A critic must challenge each mechanism before revision. Model scores do not establish improvement.",
            "seeds": [
                {"kind":"reference","text":public.to_string()},
                {"kind":"reference","text":evidence.to_string()},
                {"kind":"prior_idea","text":Value::Array(prior).to_string()},
            ],
            "constraints": {
                "must": ["Use one patch class per proposal", "Use exact language operation tags and preconditions", "Ground transcript_refs and prediction.nodes in the public dossier", "Predict verified_quality gain before evaluation"],
                "must_not": ["Rewrite immutable host verifiers or protected settings", "Invent private tasks, oracle answers or credentials", "Treat model ranking as verification or approval"],
            },
            "memory_namespace":format!("project:rsi-{hive_id}/agent:retreat"),
        },
        "output": {
            "item_name":"guarded hive improvement proposal",
            "item_guidance":"Return exactly the typed proposal fields. Each patch contains real tinyhivemind-lang operations, not prose instructions.",
            "schema":proposal_schema(),
        },
        "config": {
            "variations":3, "generations":6, "items_per_variation":3,
            "mode":"critic_pair", "template":"fugue", "temperature":1.0,
            "models":{},
            "scoring": {
                "scorers":[{"kind":"comparative","weight":2.0},{"kind":"judge","weight":1.0},{"kind":"originality","weight":1.0}],
                "select_top_k":3, "judge_raters":3, "class_threshold":0.8,
                "criteria":"Prefer grounded, falsifiable mechanisms with feasible typed edits and manageable cost; penalize repeated failed approaches and verifier changes.",
            },
            "budget":{"max_turns":6,"wall_clock_secs":120,"repair_attempts":2},
            "seed":dossier.epoch(), "include_transcripts":false,
        },
    });
    let typed: tinybackrooms::GenerateRequest =
        serde_json::from_value(request.clone()).map_err(|_| Error::InvalidProposal)?;
    typed.validate().map_err(|_| Error::InvalidProposal)?;
    Ok(request)
}

/// Builds the validated, vendored tinybackrooms request type.
///
/// # Errors
/// Returns [`Error::InvalidProposal`] for unsafe public input, malformed
/// frontmatter, or a request rejected by tinybackrooms contract validation.
pub fn typed_retreat_request(
    hive_id: &str,
    dossier: &impl RetreatDossier,
    incumbent: &Package,
    rejected: &[ProposalEvent],
) -> Result<tinybackrooms::GenerateRequest> {
    serde_json::from_value(build_retreat_request(
        hive_id, dossier, incumbent, rejected,
    )?)
    .map_err(|_| Error::InvalidProposal)
}

/// Runs the actual creative engine against the caller's model and memory ports.
///
/// Generation includes proposal/critic conversations, schema extraction,
/// comparative/judge/originality scoring and ranked selection. The engine owns
/// memory recall/storage according to its configured adapter; this module does
/// not configure persistent storage or activate any proposed edit.
///
/// # Errors
/// Returns [`Error::InvalidProposal`] for invalid public input and
/// [`Error::HostFailed`] when the creative engine or response serialization fails.
pub async fn generate_retreat_with_engine(
    engine: &tinybackrooms::Engine,
    hive_id: &str,
    dossier: &impl RetreatDossier,
    incumbent: &Package,
    rejected: &[ProposalEvent],
) -> Result<RetreatOutput> {
    let request = typed_retreat_request(hive_id, dossier, incumbent, rejected)?;
    let response = engine
        .generate(&request)
        .await
        .map_err(|_| Error::HostFailed)?;
    let response = serde_json::to_value(response).map_err(|_| Error::HostFailed)?;
    decode_retreat_response(&response)
}

/// Runs the caller's tinybackrooms transport and returns valid ranked proposals.
///
/// Use [`decode_retreat_response`] directly when discarded-item counts are
/// needed. No live package, archive head or host memory is changed here.
///
/// # Errors
/// Returns [`Error::InvalidProposal`] for invalid input or response envelopes,
/// or propagates the host transport's service error.
pub fn generate_retreat(
    transport: &impl RetreatTransport,
    hive_id: &str,
    dossier: &impl RetreatDossier,
    incumbent: &Package,
    rejected: &[ProposalEvent],
) -> Result<Vec<Proposal>> {
    let request = build_retreat_request(hive_id, dossier, incumbent, rejected)?;
    let response = transport.generate(&request)?;
    Ok(decode_retreat_response(&response)?.proposals)
}

/// Decodes tinybackrooms `items[].value`, discarding malformed proposal items.
///
/// Every nested typed object rejects unknown fields. Numeric predictions must
/// be finite and positive. Invalid raw model outputs are not copied into the
/// public archive; callers can retain them privately under their own policy.
///
/// # Errors
/// Returns [`Error::InvalidProposal`] when the response has no array of items.
pub fn decode_retreat_response(response: &Value) -> Result<RetreatOutput> {
    let items = response
        .get("items")
        .and_then(Value::as_array)
        .ok_or(Error::InvalidProposal)?;
    let mut output = RetreatOutput {
        proposals: Vec::new(),
        invalid_items: 0,
    };
    for item in items {
        let Some(value) = item.get("value") else {
            output.invalid_items += 1;
            continue;
        };
        let decoded = serde_json::from_value::<Proposal>(value.clone());
        match decoded {
            Ok(proposal) if valid_proposal(&proposal) && operation_fields_present(value) => {
                output.proposals.push(proposal);
            }
            _ => output.invalid_items += 1,
        }
    }
    Ok(output)
}

fn valid_proposal(proposal: &Proposal) -> bool {
    [
        &proposal.title,
        &proposal.mechanism,
        &proposal.risk,
        &proposal.effort,
    ]
    .iter()
    .all(|text| !text.trim().is_empty())
        && proposal.prediction.metric == "verified_quality"
        && proposal.prediction.minimum_gain.is_finite()
        && proposal.prediction.minimum_gain > 0.0
        && proposal.prediction.minimum_gain <= 1.0
        && !proposal.prediction.nodes.is_empty()
        && proposal
            .prediction
            .nodes
            .iter()
            .all(|id| !id.trim().is_empty())
        && !proposal.transcript_refs.is_empty()
        && !proposal.patch.operations.is_empty()
}

fn operation_fields_present(value: &Value) -> bool {
    value
        .pointer("/patch/operations")
        .and_then(Value::as_array)
        .is_some_and(|operations| {
            operations.iter().all(|operation| {
                operation.get("before").is_some()
                    && operation.get("after").is_some()
                    && ["path", "pointer", "item", "identity", "entry"]
                        .iter()
                        .all(|key| {
                            operation.get(key).is_none_or(|value| {
                                value.as_str().is_some_and(|text| !text.trim().is_empty())
                            })
                        })
            })
        })
}

fn public_package(package: &Package, secrets: &[String]) -> Value {
    let manifest = &package.manifest;
    let referenced = manifest
        .context
        .iter()
        .chain(manifest.roles.iter().map(|role| &role.charter))
        .chain(manifest.seats.iter().map(|seat| &seat.prompt));
    let documents: BTreeMap<_, _> = referenced
        .filter(|path| public_document(path))
        .filter_map(|path| {
            let source = package.documents.get(path)?;
            let body = source
                .strip_prefix("---\n")
                .map_or(Some(source.as_str()), |rest| {
                    rest.split_once("\n---\n").map(|(_, body)| body)
                })?;
            (!secrets
                .iter()
                .any(|secret| path.contains(secret) || body.contains(secret)))
            .then_some((path, body))
        })
        .collect();
    json!({
        "manifest": {
            "id":manifest.id, "name":manifest.name, "version":manifest.version,
            "roles":manifest.roles.iter().filter(|role| public_document(&role.charter)).map(|role|json!({"id":role.id,"charter":role.charter})).collect::<Vec<_>>(),
            "seats":manifest.seats.iter().filter(|seat| public_document(&seat.prompt)).map(|seat|json!({"id":seat.id,"label":seat.label,"role":seat.role,"prompt":seat.prompt,"model":seat.model})).collect::<Vec<_>>(),
            "desks":manifest.desks, "topology":manifest.topology, "referrals":manifest.referrals,
            "goals":manifest.goals.iter().map(|goal|json!({"id":goal.id,"description":goal.description,"subgoals":goal.subgoals})).collect::<Vec<_>>(),
            "missions":manifest.missions,
            "context":manifest.context.iter().filter(|path| public_document(path)).collect::<Vec<_>>(),
            "policies":manifest.policies, "coordinator":manifest.coordinator,
        },
        "documents":documents,
        "memory_identities":package.memory.identities.iter().filter(|identity| !package.constitution.protected_namespaces.iter().any(|prefix| identity.namespace.starts_with(prefix))).map(|identity|json!({"id":identity.id,"namespace":identity.namespace,"kind":identity.kind,"lifecycle":identity.lifecycle,"read_only":identity.read_only})).collect::<Vec<_>>(),
    })
}

fn public_document(path: &str) -> bool {
    let mut parts = path.split('/');
    matches!(parts.next(), Some("context" | "roles" | "seats"))
        && std::path::Path::new(path)
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        && parts
            .clone()
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && !path.contains('\\')
        && !parts.any(|part| {
            [
                "constitution",
                "evaluation",
                "telemetry",
                "canary",
                "oracle",
                "private",
                "acceptance",
            ]
            .iter()
            .any(|protected| part.to_ascii_lowercase().contains(protected))
        })
}

fn protected_strings(package: &Package) -> Result<Vec<String>> {
    let mut protected = package.constitution.contamination_canaries.clone();
    protected.extend(package.constitution.protected_namespaces.iter().cloned());
    protected.extend(package.constitution.hard_constraints.iter().cloned());
    protected.extend(package.constitution.read_only_memories.iter().cloned());
    for goal in &package.manifest.goals {
        protected.extend(goal.acceptance.iter().cloned());
    }
    for source in package.documents.values() {
        if let Some(rest) = source.strip_prefix("---\n") {
            let (header, _) = rest.split_once("\n---\n").ok_or(Error::InvalidProposal)?;
            let header: Value = serde_json::from_str(header).map_err(|_| Error::InvalidProposal)?;
            protected_frontmatter(&header, &mut protected);
        }
    }
    for value in [
        &package.constitution.evaluation,
        &package.constitution.acceptance,
        &package.constitution.telemetry,
    ] {
        string_leaves(value, &mut protected);
    }
    for seat in &package.manifest.seats {
        string_leaves(&seat.config, &mut protected);
    }
    protected.retain(|value| !value.is_empty());
    protected.sort_by_key(|value| std::cmp::Reverse(value.len()));
    protected.dedup();
    Ok(protected)
}
fn protected_frontmatter(value: &Value, protected: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                // Only these canonical identity/display fields enter the public
                // manifest projection. Every other header field is withheld,
                // including unknown host configuration and binding metadata.
                if matches!(
                    key.as_str(),
                    "id" | "label" | "role" | "prompt" | "charter" | "model"
                ) {
                    protected_frontmatter(value, protected);
                } else {
                    string_leaves(value, protected);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                protected_frontmatter(value, protected);
            }
        }
        _ => {}
    }
}

fn string_leaves(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => output.push(text.clone()),
        Value::Array(values) => {
            for value in values {
                string_leaves(value, output);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                string_leaves(value, output);
            }
        }
        _ => {}
    }
}
fn redact(value: &mut Value, secrets: &[String]) {
    match value {
        Value::String(text) => {
            for secret in secrets {
                *text = text.replace(secret, "[redacted]");
            }
        }
        Value::Array(values) => {
            for value in values {
                redact(value, secrets);
            }
        }
        Value::Object(values) => {
            let original = std::mem::take(values);
            for (mut key, mut value) in original {
                for secret in secrets {
                    key = key.replace(secret, "[redacted]");
                }
                redact(&mut value, secrets);
                values.insert(key, value);
            }
        }
        _ => {}
    }
}
fn public_reason(reason: &str) -> &'static str {
    match reason {
        "Untestable" => "Untestable",
        "Ungrounded" => "Ungrounded",
        "WrongClass" => "WrongClass",
        "Guard" => "Guard",
        "Unformalizable" => "Unformalizable",
        "Duplicate" => "Duplicate",
        "NoGain" => "NoGain",
        "Inconclusive" => "Inconclusive",
        "NotReplicated" => "NotReplicated",
        "ApprovalDenied" => "ApprovalDenied",
        "HostFailure" => "HostFailure",
        "Accepted" => "Accepted",
        _ => "Other",
    }
}

fn proposal_schema() -> Value {
    let text = json!({"type":"string","minLength":1});
    let nullable_text = json!({"type":["string","null"]});
    let operation = |kind: &str, mut properties: Value, fields: &[&str]| {
        properties["operation"] = json!({"const":kind});
        let required: Vec<_> = std::iter::once("operation")
            .chain(fields.iter().copied())
            .collect();
        json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
    };
    let operations = vec![
        operation(
            "json",
            json!({"pointer":text,"before":{},"after":{}}),
            &["pointer", "before", "after"],
        ),
        operation(
            "document",
            json!({"path":text,"before":nullable_text,"after":nullable_text}),
            &["path", "before", "after"],
        ),
        operation(
            "prompt",
            json!({"path":text,"before":{"type":"string"},"after":{"type":"string"}}),
            &["path", "before", "after"],
        ),
        operation(
            "context",
            json!({"path":text,"item":text,"before":nullable_text,"after":nullable_text}),
            &["path", "item", "before", "after"],
        ),
        operation(
            "learning",
            json!({"identity":text,"entry":text,"watermark":{"type":"integer","minimum":0},"before":nullable_text,"after":nullable_text}),
            &["identity", "entry", "watermark", "before", "after"],
        ),
    ];
    json!({
        "type":"object", "additionalProperties":false,
        "required":["title","patch","mechanism","prediction","transcript_refs","risk","effort"],
        "properties": {
            "title":text,"mechanism":text,"risk":text,"effort":text,
            "patch":{"type":"object","additionalProperties":false,"required":["class","operations"],"properties": {
                "class":{"enum":["prompt","context","memory","roster","topology","policy","goal"]},
                "operations":{"type":"array","minItems":1,"items":{"oneOf":operations}},
            }},
            "prediction":{"type":"object","additionalProperties":false,"required":["metric","minimum_gain","nodes"],"properties": {
                "metric":{"const":"verified_quality"},"minimum_gain":{"type":"number","exclusiveMinimum":0,"maximum":1},
                "nodes":{"type":"array","minItems":1,"items":text},
            }},
            "transcript_refs":{"type":"array","minItems":1,"items":{"type":"integer","minimum":0}},
        },
    })
}
#[cfg(test)]
mod test;
