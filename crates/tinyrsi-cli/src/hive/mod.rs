//! The hive: the loop's tinyhivemind package run as `OpenHuman` agents.
//!
//! Each epoch, [`Hive`] hands `tinyrsi-swarm` the brief, the incumbent
//! checkout (read-only for non-editing seats) and one candidate checkout per
//! editing seat. Every seat's model calls go through the run's spend gateway,
//! tagged `swarm/<seat>`, and tool keys (such as `web_search`) reach only the
//! seats they name.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use tinyrsi_swarm::{SwarmRequest, ToolKey};

use crate::{
    Error, Result,
    engine::{ProposalReport, ProposalRequest, Proposer},
    keys::Keys,
    manifest::Loop,
};

/// The hive proposer for one run.
pub struct Hive {
    package: BTreeMap<String, String>,
    lead: String,
    editing: Vec<String>,
    default_model: String,
    temperature: Option<f64>,
    tools: Vec<ToolKey>,
    wall: Duration,
    turn: Duration,
    base_url: String,
}

impl std::fmt::Debug for Hive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hive")
            .field("lead", &self.lead)
            .field("editing", &self.editing)
            .finish_non_exhaustive()
    }
}

impl Hive {
    /// The hive a loop declares, calling models through `base_url` (the
    /// gateway).
    ///
    /// # Errors
    /// Returns [`Error::Proposer`] when the package declares no editing seat,
    /// and [`Error::MissingKeys`] when a tool key has no value.
    pub fn new(parsed: &Loop, keys: &Keys, base_url: &str) -> Result<Self> {
        let editing = tinyrsi_swarm::editing_seats(&parsed.hive)
            .map_err(|e| Error::Proposer(e.to_string()))?;
        if editing.is_empty() {
            return Err(Error::Proposer(
                "no seat edits a candidate: set a seat's config.action_dir to \"{candidate}\""
                    .into(),
            ));
        }
        let mut tools = Vec::new();
        for (capability, tool) in parsed.tool_keys() {
            let secret = keys.get(&tool.env).ok_or_else(|| Error::MissingKeys {
                names: vec![tool.env.clone()],
            })?;
            tools.push(ToolKey {
                capability: capability.to_owned(),
                provider: tool.provider.clone(),
                secret: secret.expose().to_owned(),
                seats: tool.seats.clone(),
            });
        }
        let m = &parsed.manifest;
        Ok(Self {
            package: parsed.hive.clone(),
            lead: m.hive.lead.clone(),
            editing,
            default_model: parsed.seat_model().to_owned(),
            temperature: m.creativity.temperature,
            tools,
            wall: Duration::from_secs(m.hive.wall_secs),
            turn: Duration::from_secs(m.hive.turn_secs),
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }
}

impl Proposer for Hive {
    fn editing_seats(&self) -> Vec<String> {
        self.editing.clone()
    }

    fn propose(&mut self, request: &ProposalRequest<'_>) -> Result<ProposalReport> {
        let base = self.base_url.clone();
        let outcome = tinyrsi_swarm::run_swarm(
            SwarmRequest {
                package: self.package.clone(),
                lead: self.lead.clone(),
                brief: request.brief.to_owned(),
                run_dir: request.dir.to_path_buf(),
                incumbent: request.incumbent.path.clone(),
                candidates: request
                    .candidates
                    .iter()
                    .map(|(seat, checkout)| (seat.clone(), checkout.path.clone()))
                    .collect(),
                model_base_url: Arc::new(move |seat| format!("{base}/__tag/swarm/{seat}/v1")),
                model_api_key: "gateway".into(),
                default_model: self.default_model.clone(),
                temperature: self.temperature,
                tools: self.tools.clone(),
                wall: self.wall,
                turn: self.turn,
            },
            &mut |_| {},
        )
        .map_err(|e| Error::Proposer(e.to_string()))?;
        Ok(ProposalReport {
            summary: Some(format!(
                "stopped {}: {} turns completed, {} failed, {} parked",
                serde_json::to_string(&outcome.stop)
                    .unwrap_or_default()
                    .trim_matches('"'),
                outcome.completed,
                outcome.failed,
                outcome.parked
            )),
            seats: outcome
                .seats
                .iter()
                .map(|(seat, usage)| {
                    (
                        seat.clone(),
                        format!(
                            "{} turns, {} in / {} out tokens, ${:.4}",
                            usage.turns, usage.input_tokens, usage.output_tokens, usage.cost_usd
                        ),
                    )
                })
                .collect(),
        })
    }
}
