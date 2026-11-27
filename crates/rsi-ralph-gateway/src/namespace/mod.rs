//! Codex's grouped tools, offered upstream as plain functions.
//!
//! Codex sends the tools of each MCP server as one Responses `namespace`
//! tool (`{"type": "namespace", "name": "mcp__hive", "tools": [..]}`) and
//! expects a call to one of them back as a `function_call` naming the tool
//! and its `namespace`. `OpenRouter` drops namespace tools, which leaves the
//! model none of the server's tools. [`flatten`] offers each as a plain
//! function named `<namespace>__<tool>`, and renames the calls already in the
//! input the same way; [`unflatten`] turns a call to such a function back
//! into Codex's form, in a whole response or, through [`Rewriter`], in a
//! stream as it passes.

use axum::body::Bytes;
use serde_json::{Map, Value};

/// Joins a namespace and a tool into a plain function name.
pub(crate) const SEPARATOR: &str = "__";

/// Offers every `namespace` tool in `body` as plain functions, renames the
/// namespaced calls in its `input`, and returns the namespaces seen.
pub(crate) fn flatten(body: &mut Map<String, Value>) -> Vec<String> {
    let mut namespaces: Vec<String> = Vec::new();
    if let Some(Value::Array(tools)) = body.get_mut("tools") {
        let mut flat = Vec::with_capacity(tools.len());
        for tool in tools.drain(..) {
            if tool.get("type").and_then(Value::as_str) != Some("namespace") {
                flat.push(tool);
                continue;
            }
            let name = tool.get("name").and_then(Value::as_str).unwrap_or("");
            remember(&mut namespaces, name);
            let inner = tool.get("tools").and_then(Value::as_array);
            for function in inner.into_iter().flatten() {
                let mut function = function.clone();
                let tool_name = function.get("name").and_then(Value::as_str).unwrap_or("");
                let joined = format!("{name}{SEPARATOR}{tool_name}");
                function["type"] = "function".into();
                function["name"] = joined.into();
                flat.push(function);
            }
        }
        *tools = flat;
    }
    if let Some(Value::Array(input)) = body.get_mut("input") {
        for item in input.iter_mut().filter_map(Value::as_object_mut) {
            let Some(Value::String(namespace)) = item.remove("namespace") else {
                continue;
            };
            let tool = item.get("name").and_then(Value::as_str).unwrap_or("");
            let joined = format!("{namespace}{SEPARATOR}{tool}");
            item.insert("name".into(), joined.into());
            remember(&mut namespaces, &namespace);
        }
    }
    namespaces
}

fn remember(namespaces: &mut Vec<String>, namespace: &str) {
    if !namespaces.iter().any(|seen| seen == namespace) {
        namespaces.push(namespace.to_owned());
    }
}

/// Names every `function_call` to a flattened function in `value` back in
/// Codex's form: the tool's own name and its `namespace`.
pub(crate) fn unflatten(value: &mut Value, namespaces: &[String]) {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("function_call")
                && let Some(Value::String(name)) = map.get("name")
                && let Some((namespace, tool)) = split(name, namespaces)
            {
                map.insert("name".into(), tool.into());
                map.insert("namespace".into(), namespace.into());
            }
            map.values_mut()
                .for_each(|value| unflatten(value, namespaces));
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|value| unflatten(value, namespaces)),
        _ => {}
    }
}

/// The namespace and tool a flattened function name joins, if it joins one
/// of `namespaces`.
fn split(name: &str, namespaces: &[String]) -> Option<(String, String)> {
    namespaces.iter().find_map(|namespace| {
        let tool = name
            .strip_prefix(namespace.as_str())?
            .strip_prefix(SEPARATOR)?;
        Some((namespace.clone(), tool.to_owned()))
    })
}

/// [`unflatten`] over a whole JSON body; any other body is returned as it
/// came.
pub(crate) fn unflatten_body(body: Bytes, namespaces: &[String]) -> Bytes {
    if namespaces.is_empty() {
        return body;
    }
    match serde_json::from_slice::<Value>(&body) {
        Ok(mut value) => {
            unflatten(&mut value, namespaces);
            serde_json::to_vec(&value).map_or(body, Bytes::from)
        }
        Err(_) => body,
    }
}

/// Rewrites a server-sent-event stream line by line, naming flattened calls
/// back as they pass. Only whole lines are rewritten; a partial line waits
/// for the rest.
#[derive(Debug)]
pub(crate) struct Rewriter {
    namespaces: Vec<String>,
    pending: Vec<u8>,
}

impl Rewriter {
    /// A rewriter for `namespaces`.
    pub(crate) fn new(namespaces: Vec<String>) -> Self {
        Self {
            namespaces,
            pending: Vec::new(),
        }
    }

    /// The rewritten bytes of every line `chunk` completes. With no
    /// namespaces the chunk passes as it came.
    pub(crate) fn feed(&mut self, chunk: Bytes) -> Bytes {
        if self.namespaces.is_empty() {
            return chunk;
        }
        self.pending.extend_from_slice(&chunk);
        let Some(end) = self.pending.iter().rposition(|&b| b == b'\n') else {
            return Bytes::new();
        };
        let lines: Vec<u8> = self.pending.drain(..=end).collect();
        let mut out = Vec::with_capacity(lines.len());
        for line in lines.split_inclusive(|&b| b == b'\n') {
            out.extend_from_slice(&self.line(line));
        }
        Bytes::from(out)
    }

    /// Whatever is left of the stream, rewritten.
    pub(crate) fn finish(&mut self) -> Bytes {
        let rest = std::mem::take(&mut self.pending);
        Bytes::from(self.line(&rest))
    }

    fn line(&self, line: &[u8]) -> Vec<u8> {
        let Ok(text) = std::str::from_utf8(line) else {
            return line.to_vec();
        };
        let Some(data) = text.strip_prefix("data:") else {
            return line.to_vec();
        };
        if !data.contains("function_call") {
            return line.to_vec();
        }
        let Ok(mut event) = serde_json::from_str::<Value>(data.trim()) else {
            return line.to_vec();
        };
        unflatten(&mut event, &self.namespaces);
        let ending = &text[text.trim_end_matches(['\r', '\n']).len()..];
        format!("data: {event}{ending}").into_bytes()
    }
}

#[cfg(test)]
mod test;
