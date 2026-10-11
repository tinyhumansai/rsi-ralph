//! The gateway's model listing, carrying the upstream's size limits.
//!
//! The gateway lists only the models its price table allows, which is the
//! point of it: a seat should see what it may call, not the upstream's whole
//! catalog. But an id is not all a client reads there. Agent harnesses size
//! their context management from the listing's `context_length` (and its
//! relatives), and one that finds none has to assume the worst. OpenHuman's
//! assumption is to blank old tool results on every turn, rewriting history
//! the provider already cached, so a hive behind this gateway missed its
//! prompt cache on 77 of 80 builder turns (measured 2026-10-12).
//!
//! So each allowed model is listed with the limit fields the upstream reports
//! for it. The upstream listing is read best effort: a slow or failing one
//! leaves the ids as they were.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::{Map, Value, json};

use super::Proxy;

/// How long the upstream listing may take before the ids are served bare.
const LISTING_TIMEOUT: Duration = Duration::from_secs(5);

/// The fields copied from an upstream listing entry: the ones OpenAI-
/// compatible clients read a model's window and output cap from. Prices and
/// descriptions stay behind; the gateway's own table decides those.
const LIMIT_FIELDS: [&str; 8] = [
    "context_length",
    "context_window",
    "max_context_length",
    "max_model_len",
    "max_input_tokens",
    "max_output_tokens",
    "max_completion_tokens",
    "top_provider",
];

/// The listing body: every allowed model, with its upstream limits.
pub(super) async fn listing(proxy: &Proxy) -> Value {
    let upstream = upstream_limits(proxy).await;
    let data: Vec<Value> = proxy
        .config
        .prices
        .models()
        .map(|id| entry(id, upstream.get(id)))
        .collect();
    json!({ "object": "list", "data": data })
}

/// One listing entry: the id, plus whatever limits the upstream gave it.
fn entry(id: &str, limits: Option<&Map<String, Value>>) -> Value {
    let mut entry = Map::new();
    entry.insert("id".to_owned(), id.into());
    entry.insert("object".to_owned(), "model".into());
    if let Some(limits) = limits {
        entry.extend(limits.clone());
    }
    Value::Object(entry)
}

/// The upstream's limit fields per model id; empty when it cannot be read.
async fn upstream_limits(proxy: &Proxy) -> HashMap<String, Map<String, Value>> {
    let url = format!("{}/models", proxy.config.upstream.trim_end_matches('/'));
    let sent = proxy
        .client
        .get(url)
        .bearer_auth(&proxy.config.api_key)
        .timeout(LISTING_TIMEOUT)
        .send()
        .await;
    let body = match sent {
        Ok(response) if response.status().is_success() => response.json::<Value>().await.ok(),
        _ => None,
    };
    body.map(|body| limits_by_id(&body)).unwrap_or_default()
}

/// Reads an OpenAI-shaped listing (`{"data": [{"id": ..}, ..]}`) into the
/// limit fields of each entry that has any.
fn limits_by_id(body: &Value) -> HashMap<String, Map<String, Value>> {
    let Some(entries) = body.get("data").and_then(Value::as_array) else {
        return HashMap::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_str()?;
            let limits: Map<String, Value> = LIMIT_FIELDS
                .iter()
                .filter_map(|field| Some(((*field).to_owned(), entry.get(*field)?.clone())))
                .collect();
            (!limits.is_empty()).then(|| (id.to_owned(), limits))
        })
        .collect()
}

#[cfg(test)]
#[path = "models_test.rs"]
mod test;
