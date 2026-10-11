//! Strict host-private configuration; none of these types implement debug output.
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bank {
    pub version: u32,
    pub provenance: String,
    pub private_seed: String,
    pub stages: BTreeMap<String, Vec<Row>>,
    pub calibration: Vec<Sample>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Sample {
    pub task_id: String,
    pub artifact: String,
    pub passed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Row {
    pub id: String,
    pub family: String,
    pub seat: String,
    pub prompt: String,
    pub expected: String,
    pub seeds: Vec<u64>,
}
