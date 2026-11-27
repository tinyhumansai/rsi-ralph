//! Reading what a call actually cost from the upstream response.
//!
//! The upstream reports `usage.cost` in dollars when asked. Surplus
//! Intelligence sends `usage.buyer_cost_micro` instead, an amount in
//! micro-dollars, which is used when `usage.cost` is absent. A non-streaming
//! response carries `usage` at the top level; a stream carries it in its
//! `data:` events, which [`SseUsage`] finds by scanning the bytes as they
//! pass through. Chat Completions names the counts `prompt_tokens` and
//! `completion_tokens`; Anthropic's Messages and `OpenAI`'s Responses name
//! them `input_tokens` and `output_tokens`. A Responses stream reports usage
//! inside `response`, and a Messages stream splits it: the input count in
//! `message_start`'s `message`, the output count in `message_delta`. The
//! scanner keeps the largest of each count it sees, which joins the split
//! and is the final count for every API. When the cost is missing but token counts are
//! present, [`Usage::cost_or_ceiling`] prices the tokens at the model's
//! ceiling, which is still an upper bound. Cached and reasoning token counts
//! are recorded when the provider reports them and left unknown otherwise;
//! neither changes the cost, since no cache discount is assumed. The same
//! scan notes when the first generated token arrives, which the proxy turns
//! into time to first token.

use serde_json::Value;

use crate::price::{Price, usd};

/// Token counts and cost from one response.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Usage {
    /// Provider-reported cost in dollars: `usage.cost`, else
    /// `usage.buyer_cost_micro` converted from micro-dollars.
    pub(crate) cost: Option<f64>,
    /// Prompt tokens.
    pub(crate) prompt_tokens: u64,
    /// Completion tokens, including reasoning.
    pub(crate) completion_tokens: u64,
    /// Prompt tokens the provider served from its prompt cache
    /// (`usage.prompt_tokens_details.cached_tokens`). `None` when the
    /// provider did not say, which is not the same as no hits.
    pub(crate) cached_tokens: Option<u64>,
    /// Completion tokens spent reasoning
    /// (`usage.completion_tokens_details.reasoning_tokens`). `None` when the
    /// provider did not say.
    pub(crate) reasoning_tokens: Option<u64>,
    /// Whether any usage object was seen at all.
    pub(crate) seen: bool,
}

impl Usage {
    /// Reads `usage` from a response body or a stream event: at the top
    /// level, or inside a Responses event's `response` or a Messages
    /// event's `message`.
    #[must_use]
    pub(crate) fn from_body(body: &Value) -> Self {
        let Some(usage) = ["/usage", "/response/usage", "/message/usage"]
            .iter()
            .find_map(|at| body.pointer(at).filter(|u| u.is_object()))
        else {
            return Self::default();
        };
        let count = |keys: &[&str]| {
            keys.iter()
                .find_map(|key| usage.pointer(key).and_then(Value::as_u64))
        };
        // Anthropic counts cache reads and writes apart from `input_tokens`.
        let anthropic_cache = count(&["/cache_read_input_tokens"]).unwrap_or(0)
            + count(&["/cache_creation_input_tokens"]).unwrap_or(0);
        Self {
            cost: usage
                .get("cost")
                .and_then(Value::as_f64)
                .or_else(|| buyer_cost(usage)),
            prompt_tokens: count(&["/prompt_tokens", "/input_tokens"]).unwrap_or(0)
                + anthropic_cache,
            completion_tokens: count(&["/completion_tokens", "/output_tokens"]).unwrap_or(0),
            cached_tokens: count(&[
                "/prompt_tokens_details/cached_tokens",
                "/input_tokens_details/cached_tokens",
                "/cache_read_input_tokens",
            ]),
            reasoning_tokens: count(&[
                "/completion_tokens_details/reasoning_tokens",
                "/output_tokens_details/reasoning_tokens",
            ]),
            seen: true,
        }
    }

    /// The largest of each count in `self` and `other`, and `other`'s cost
    /// when it has one.
    fn merge(&mut self, other: &Self) {
        self.cost = other.cost.or(self.cost);
        self.prompt_tokens = self.prompt_tokens.max(other.prompt_tokens);
        self.completion_tokens = self.completion_tokens.max(other.completion_tokens);
        self.cached_tokens = self.cached_tokens.max(other.cached_tokens);
        self.reasoning_tokens = self.reasoning_tokens.max(other.reasoning_tokens);
        self.seen = true;
    }

    /// The reported cost, else the tokens priced at the ceiling, else `None`.
    #[must_use]
    pub(crate) fn cost_or_ceiling(&self, price: &Price) -> Option<f64> {
        self.cost.or_else(|| {
            self.seen.then_some(
                usd(self.prompt_tokens, price.prompt)
                    + usd(self.completion_tokens, price.completion),
            )
        })
    }
}

/// `usage.buyer_cost_micro` as dollars, when it is a finite, non-negative
/// number.
fn buyer_cost(usage: &Value) -> Option<f64> {
    let micro = usage.get("buyer_cost_micro").and_then(Value::as_f64)?;
    (micro.is_finite() && micro >= 0.0).then_some(micro / 1e6)
}

/// Incremental scanner for the usage event in a server-sent-event stream.
#[derive(Debug, Default)]
pub(crate) struct SseUsage {
    pending: Vec<u8>,
    usage: Usage,
    token: bool,
}

impl SseUsage {
    /// Feeds the next chunk of stream bytes.
    pub(crate) fn feed(&mut self, chunk: &[u8]) {
        self.pending.extend_from_slice(chunk);
        while let Some(end) = self.pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            self.line(&line);
        }
    }

    /// Whether a complete event carrying generated text, reasoning or a
    /// tool call has been seen. Keepalive comments and empty role deltas do
    /// not count.
    #[must_use]
    pub(crate) fn saw_token(&self) -> bool {
        self.token
    }

    /// Finishes the stream and returns its usage: the largest of each count
    /// seen, and the last cost.
    #[must_use]
    pub(crate) fn finish(mut self) -> Usage {
        let rest = std::mem::take(&mut self.pending);
        self.line(&rest);
        self.usage
    }

    fn line(&mut self, line: &[u8]) {
        let Ok(text) = std::str::from_utf8(line) else {
            return;
        };
        let Some(data) = text.trim().strip_prefix("data:") else {
            return;
        };
        if let Ok(event) = serde_json::from_str::<Value>(data.trim()) {
            self.token = self.token || carries_token(&event);
            let usage = Usage::from_body(&event);
            if usage.seen {
                self.usage.merge(&usage);
            }
        }
    }
}

/// Whether a stream event holds any generated output: a chat delta with
/// content, reasoning or tool calls, a Messages content block delta, or a
/// Responses text, reasoning or call-argument delta.
fn carries_token(event: &Value) -> bool {
    let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
    if kind == "content_block_delta" {
        return true;
    }
    if kind.starts_with("response.") && kind.rsplit('.').next() == Some("delta") {
        return event
            .get("delta")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty());
    }
    let Some(delta) = event.pointer("/choices/0/delta") else {
        return false;
    };
    let text = |key: &str| {
        delta
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
    };
    text("content")
        || text("reasoning")
        || text("reasoning_content")
        || delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty())
}

#[cfg(test)]
mod test;
