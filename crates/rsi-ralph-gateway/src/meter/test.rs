//! Unit tests for the meter and the stored form of its rows.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use super::{Meter, MeterRow, Tags};
use crate::error::Error;

fn row() -> MeterRow {
    MeterRow {
        at_ms: 1,
        tags: Tags {
            phase: "score".into(),
            seat: "judge".into(),
            candidate: String::new(),
        },
        model: "m".into(),
        endpoint: "chat/completions".into(),
        status: 200,
        stream: false,
        reserved_usd: 0.1,
        cost_usd: Some(0.01),
        prompt_tokens: 1,
        completion_tokens: 2,
        wall_ms: 3,
        ttft_ms: Some(2),
        error: None,
    }
}

#[test]
fn pins_the_stored_row_shape() {
    assert_eq!(
        serde_json::to_value(row()).unwrap(),
        json!({
            "at_ms": 1,
            "phase": "score",
            "seat": "judge",
            "candidate": "",
            "model": "m",
            "endpoint": "chat/completions",
            "status": 200,
            "stream": false,
            "reserved_usd": 0.1,
            "cost_usd": 0.01,
            "prompt_tokens": 1,
            "completion_tokens": 2,
            "wall_ms": 3,
            "ttft_ms": 2,
            "error": null,
        })
    );
}

#[test]
fn appends_flat_json_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/meter.jsonl");
    let meter = Meter::open(&path).unwrap();
    meter.record(&row()).unwrap();
    meter.record(&row()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["seat"], "judge");
    assert_eq!(lines[0]["phase"], "score");
}

#[test]
fn reports_unwritable_meters() {
    let dir = tempfile::tempdir().unwrap();
    let meter = Meter::open(dir.path()).unwrap();
    assert!(matches!(
        meter.record(&row()).unwrap_err(),
        Error::Io { .. }
    ));
}

#[test]
fn reports_an_uncreatable_meter_directory() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a-file");
    std::fs::write(&file, "").unwrap();
    assert!(matches!(
        Meter::open(&file.join("meter.jsonl")).unwrap_err(),
        Error::Io { .. }
    ));
}
