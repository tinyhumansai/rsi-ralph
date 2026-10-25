//! Reading what a call actually cost from the upstream response.
//!
//! The upstream reports `usage.cost` in dollars when asked. A non-streaming
//! response carries `usage` at the top level; a stream carries it in one of
//! its final `data:` events, which [`SseUsage`] finds by scanning the bytes
//! as they pass through. When the cost is missing but token counts are
//! present, [`Usage::cost_or_ceiling`] prices the tokens at the model's
//! ceiling, which is still an upper bound. The same scan notes when the
//! first generated token arrives, which the proxy turns into time to first
//! token.

use serde_json::Value;

use crate::price::{Price, usd};

/// Token counts and cost from one response.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Usage {
    /// Provider-reported cost in dollars.
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
    /// Reads `usage` from a response body or a stream event.
    #[must_use]
    pub(crate) fn from_body(body: &Value) -> Self {
        let Some(usage) = body.get("usage").filter(|u| u.is_object()) else {
            return Self::default();
        };
        Self {
            cost: usage.get("cost").and_then(Value::as_f64),
            prompt_tokens: usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            completion_tokens: usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            cached_tokens: None,
            reasoning_tokens: None,
            seen: true,
        }
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

    /// Finishes the stream and returns the last usage seen.
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
                self.usage = usage;
            }
        }
    }
}

/// Whether a stream event's first choice delta holds any generated output.
fn carries_token(event: &Value) -> bool {
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
