//! Private slice configuration and safe observed-check metadata.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Slice {
    pub version: u32,
    pub private_seed: String,
    pub toolchain: String,
    pub stages: BTreeMap<String, Vec<Goal>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Goal {
    pub id: String,
    pub theorem: String,
    pub seed: u64,
}

#[derive(Serialize)]
pub(super) struct Receipt {
    pub task_id: String,
    pub passed: bool,
    pub exit_code: Option<i32>,
}
