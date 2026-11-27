//! Flattening Codex's namespace tools and naming their calls back.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use serde_json::json;

fn codex_body() -> Map<String, Value> {
    let body = json!({
        "model": "openai/gpt-5.6-luna",
        "tools": [
            {"type": "function", "name": "read_mcp_resource", "parameters": {}},
            {"type": "namespace", "name": "mcp__hive", "description": "the hive",
             "tools": [
                {"type": "function", "name": "read_file", "description": "read",
                 "parameters": {"type": "object"}},
                {"type": "function", "name": "hivemind_post", "parameters": {}},
             ]},
        ],
        "input": [
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "go"}]},
            {"type": "function_call", "namespace": "mcp__hive", "name": "read_file",
             "arguments": "{}", "call_id": "c1"},
            {"type": "function_call_output", "call_id": "c1", "output": "x"},
        ],
    });
    let Value::Object(map) = body else {
        unreachable!("the fixture is an object")
    };
    map
}

#[test]
fn namespace_tools_become_plain_functions_and_calls_are_renamed() {
    let mut body = codex_body();
    let namespaces = flatten(&mut body);
    assert_eq!(namespaces, vec!["mcp__hive".to_owned()]);
    let names: Vec<&str> = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "read_mcp_resource",
            "mcp__hive__read_file",
            "mcp__hive__hivemind_post"
        ]
    );
    let read = &body["tools"][1];
    assert_eq!(read["type"], "function");
    assert_eq!(read["description"], "read");
    assert_eq!(read["parameters"], json!({"type": "object"}));
    let call = &body["input"][1];
    assert_eq!(call["name"], "mcp__hive__read_file");
    assert!(call.get("namespace").is_none());
    // Everything else is left alone.
    assert_eq!(body["input"][0]["role"], "user");
    assert_eq!(body["input"][2]["output"], "x");
}

#[test]
fn a_body_without_namespaces_is_unchanged() {
    let Value::Object(mut body) =
        json!({"tools": [{"type": "function", "name": "f"}], "input": "hi"})
    else {
        unreachable!("the fixture is an object")
    };
    let before = body.clone();
    assert_eq!(flatten(&mut body), Vec::<String>::new());
    assert_eq!(body, before);
}

#[test]
fn a_namespace_seen_only_in_the_input_is_still_remembered() {
    let Value::Object(mut body) = json!({"input": [
        {"type": "function_call", "namespace": "mcp__x", "name": "t", "call_id": "c"}
    ]}) else {
        unreachable!("the fixture is an object")
    };
    assert_eq!(flatten(&mut body), vec!["mcp__x".to_owned()]);
    assert_eq!(body["input"][0]["name"], "mcp__x__t");
}

#[test]
fn calls_to_flattened_functions_are_named_back() {
    let namespaces = vec!["mcp__hive".to_owned()];
    let mut response = json!({"output": [
        {"type": "function_call", "name": "mcp__hive__write_file", "arguments": "{}"},
        {"type": "function_call", "name": "read_mcp_resource", "arguments": "{}"},
        {"type": "message", "name": "mcp__hive__not_a_call"},
    ]});
    unflatten(&mut response, &namespaces);
    assert_eq!(response["output"][0]["name"], "write_file");
    assert_eq!(response["output"][0]["namespace"], "mcp__hive");
    assert_eq!(response["output"][1]["name"], "read_mcp_resource");
    assert!(response["output"][1].get("namespace").is_none());
    assert_eq!(response["output"][2]["name"], "mcp__hive__not_a_call");
}

#[test]
fn a_whole_body_is_rewritten_only_when_namespaces_were_flattened() {
    let namespaces = vec!["mcp__hive".to_owned()];
    let body =
        Bytes::from(r#"{"output":[{"type":"function_call","name":"mcp__hive__read_file"}]}"#);
    let rewritten: Value =
        serde_json::from_slice(&unflatten_body(body.clone(), &namespaces)).unwrap();
    assert_eq!(rewritten["output"][0]["name"], "read_file");
    assert_eq!(unflatten_body(body.clone(), &[]), body);
    let text = Bytes::from("not json");
    assert_eq!(unflatten_body(text.clone(), &namespaces), text);
}

#[test]
fn a_stream_is_rewritten_line_by_line_across_chunks() {
    let mut rewriter = Rewriter::new(vec!["mcp__hive".to_owned()]);
    let event = r#"data: {"type":"response.output_item.done","item":{"type":"function_call","name":"mcp__hive__read_file"}}"#;
    let (first, second) = event.split_at(40);
    // A partial line waits for the rest.
    assert_eq!(rewriter.feed(Bytes::from(first.to_owned())), Bytes::new());
    let out = rewriter.feed(Bytes::from(format!("{second}\n\nevent: done\n")));
    let text = String::from_utf8(out.to_vec()).unwrap();
    let mut lines = text.lines();
    let data: Value =
        serde_json::from_str(lines.next().unwrap().strip_prefix("data: ").unwrap()).unwrap();
    assert_eq!(data["item"]["name"], "read_file");
    assert_eq!(data["item"]["namespace"], "mcp__hive");
    assert_eq!(lines.next(), Some(""));
    assert_eq!(lines.next(), Some("event: done"));
    // Lines with no call, broken JSON and non-UTF-8 pass as they came.
    let plain = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hi\"}\n";
    assert_eq!(rewriter.feed(Bytes::from(plain)), Bytes::from(plain));
    let broken = "data: {\"function_call\n";
    assert_eq!(rewriter.feed(Bytes::from(broken)), Bytes::from(broken));
    let binary = Bytes::from_static(b"\xff\xfe\n");
    assert_eq!(rewriter.feed(binary.clone()), binary);
    // An unterminated last line comes out at the finish.
    assert_eq!(rewriter.feed(Bytes::from("data: [DONE]")), Bytes::new());
    assert_eq!(rewriter.finish(), Bytes::from("data: [DONE]"));
}

#[test]
fn a_stream_with_no_namespaces_passes_untouched() {
    let mut rewriter = Rewriter::new(Vec::new());
    let chunk = Bytes::from("data: {\"type\":\"function_call\",\"name\":\"a__b\"}");
    assert_eq!(rewriter.feed(chunk.clone()), chunk);
    assert_eq!(rewriter.finish(), Bytes::new());
}
