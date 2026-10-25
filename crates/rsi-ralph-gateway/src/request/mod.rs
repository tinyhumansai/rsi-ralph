//! Turning a client's request into the request the gateway forwards.
//!
//! [`prepare`] validates the model against the [`PriceTable`], rejects
//! content the run configuration does not support (images), bounds the
//! completion length, pins the provider and its price ceiling, asks
//! the upstream to report cost, and computes the worst-case dollar cost the
//! call reserves before it is sent.

use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::price::{ANY_PROVIDER, PriceTable, usd};

/// Prompt-side slack added to the byte count when bounding a call's cost.
/// Bytes over-count tokens for text; the slack covers templates a provider
/// adds around the messages.
pub(crate) const PROMPT_SLACK_TOKENS: u64 = 20_000;

/// Which endpoint a request targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Endpoint {
    /// `/v1/chat/completions`.
    Chat,
    /// `/v1/embeddings`.
    Embeddings,
}

impl Endpoint {
    /// The upstream path below the base URL.
    #[must_use]
    pub(crate) fn path(self) -> &'static str {
        match self {
            Self::Chat => "chat/completions",
            Self::Embeddings => "embeddings",
        }
    }
}

/// Completion-length limits applied to chat requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Used when the client sets no `max_tokens`.
    pub default_max_tokens: u64,
    /// The largest `max_tokens` the gateway forwards.
    pub max_tokens_cap: u64,
    /// Reasoning effort applied when the client names none, sent both as
    /// `reasoning.effort` and as `reasoning_effort`. Hidden reasoning is
    /// billed as completion tokens, so the default pins it to `low`; `None`
    /// forwards the client's choice untouched.
    pub default_reasoning: Option<&'static str>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            default_max_tokens: 8_192,
            max_tokens_cap: 16_384,
            default_reasoning: Some("low"),
        }
    }
}

/// A request ready to forward.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Prepared {
    /// The rewritten JSON body.
    pub(crate) body: Value,
    /// The model id.
    pub(crate) model: String,
    /// Whether the client asked for a server-sent-event stream.
    pub(crate) stream: bool,
    /// The worst-case cost in dollars.
    pub(crate) reserve_usd: f64,
}

/// Validates and rewrites `body` for `endpoint`.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] for a body that is not a JSON object, has no
/// model, or carries non-text content, and [`Error::UnknownModel`] for a model
/// not in `prices`.
pub(crate) fn prepare(
    endpoint: Endpoint,
    body: Value,
    prices: &PriceTable,
    limits: Limits,
) -> Result<Prepared> {
    let Value::Object(mut map) = body else {
        return Err(Error::BadRequest("body must be a JSON object".into()));
    };
    let model = map
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::BadRequest("missing model".into()))?
        .to_owned();
    let price = prices.get(&model)?;
    let original_bytes = serde_json::to_vec(&map).map_or(0, |bytes| bytes.len()) as u64;
    let prompt_bound = usd(original_bytes + PROMPT_SLACK_TOKENS, price.prompt);
    if price.provider != ANY_PROVIDER {
        map.insert(
            "provider".into(),
            json!({
                "only": [price.provider],
                "allow_fallbacks": false,
                "max_price": { "prompt": price.prompt * 1e6, "completion": price.completion * 1e6 },
            }),
        );
    }
    let mut stream = false;
    let mut reserve_usd = prompt_bound;
    if endpoint == Endpoint::Chat {
        reject_non_text(map.get("messages"))?;
        let requested = map
            .remove("max_completion_tokens")
            .or_else(|| map.get("max_tokens").cloned())
            .and_then(|v| v.as_u64())
            .unwrap_or(limits.default_max_tokens);
        let max_tokens = requested.clamp(1, limits.max_tokens_cap);
        map.insert("max_tokens".into(), json!(max_tokens));
        map.insert("usage".into(), json!({ "include": true }));
        // OpenRouter reads `reasoning.effort`; OpenAI-compatible upstreams
        // such as Surplus Intelligence read only `reasoning_effort`, and
        // reason at full length without it. Keep an explicit client choice,
        // in either spelling, else the default, and send it in both.
        let client_reasoning = map.get("reasoning").cloned();
        let effort = map
            .remove("reasoning_effort")
            .and_then(|v| v.as_str().map(str::to_owned))
            .or_else(|| {
                client_reasoning
                    .as_ref()
                    .and_then(|r| r.get("effort"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .or_else(|| match client_reasoning {
                None => limits.default_reasoning.map(str::to_owned),
                Some(_) => None,
            });
        if let Some(effort) = effort {
            if client_reasoning.is_none() {
                map.insert("reasoning".into(), json!({ "effort": effort }));
            }
            map.insert("reasoning_effort".into(), json!(effort));
        }
        stream = map.get("stream").and_then(Value::as_bool).unwrap_or(false);
        if stream {
            map.insert("stream_options".into(), json!({ "include_usage": true }));
        }
        reserve_usd += usd(max_tokens, price.completion);
    }
    Ok(Prepared {
        body: Value::Object(map),
        model,
        stream,
        reserve_usd,
    })
}

fn reject_non_text(messages: Option<&Value>) -> Result<()> {
    let Some(messages) = messages.and_then(Value::as_array) else {
        return Err(Error::BadRequest(
            "chat requests need a messages array".into(),
        ));
    };
    for message in messages {
        if let Some(parts) = message.get("content").and_then(Value::as_array) {
            for part in parts {
                if part.get("type").and_then(Value::as_str) != Some("text") {
                    return Err(Error::BadRequest(
                        "only text content is supported in this run configuration".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod test;
