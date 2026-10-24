//! A lowered hive and the per-seat facts the swarm builds agents from.

use std::collections::BTreeMap;

use tinyhivemind_lang::{LoweredHive, LoweredSeat};

use super::CANDIDATE_DIR;
use crate::Result;

/// A parsed, validated and lowered hive package.
#[derive(Clone, Debug)]
pub(crate) struct Hive {
    /// The language's activation snapshot: desks, context, coordinator.
    pub lowered: LoweredHive,
    /// Every seat in manifest order, with its role and charter.
    pub seats: Vec<SeatPlan>,
}

/// One seat and what the language keeps apart from it.
#[derive(Clone, Debug)]
pub(crate) struct SeatPlan {
    /// The lowered seat: prompt body, model, tools and config.
    pub seat: LoweredSeat,
    /// The role this seat fills.
    pub role: String,
    /// The role's charter body, without frontmatter.
    pub charter: String,
    /// Whether the seat edits a candidate (`config.action_dir`).
    pub editing: bool,
}

impl Hive {
    /// Parses, validates and lowers `files`.
    pub(crate) fn open(files: &BTreeMap<String, String>) -> Result<Self> {
        let package = tinyhivemind_lang::parse(files)?;
        let lowered = tinyhivemind_lang::lower(&package)?;
        let manifest = &package.manifest;
        let seats = lowered
            .seats
            .iter()
            .map(|seat| {
                let role = manifest
                    .seats
                    .iter()
                    .find(|declared| declared.id == seat.id)
                    .map(|declared| declared.role.clone())
                    .unwrap_or_default();
                let charter = manifest
                    .roles
                    .iter()
                    .find(|declared| declared.id == role)
                    .and_then(|declared| package.documents.get(&declared.charter))
                    .map(|text| body(text).to_owned())
                    .unwrap_or_default();
                SeatPlan {
                    editing: seat.config["action_dir"] == CANDIDATE_DIR,
                    seat: seat.clone(),
                    role,
                    charter,
                }
            })
            .collect();
        Ok(Self { lowered, seats })
    }
}

/// A document's body: the text after a `---` frontmatter block, if any.
fn body(text: &str) -> &str {
    text.strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map_or(text, |(_, body)| body)
}
