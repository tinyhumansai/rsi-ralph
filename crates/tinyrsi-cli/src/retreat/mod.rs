//! The creative retreat: tinybackrooms conversations that seed each brief.
//!
//! Before the hive works, the retreat runs `variations` independent
//! conversations of `turns` turns over the brief, extracts typed ideas from
//! them, ranks them for novelty, and hands the best `ideas` to the brief's
//! `{{ideas}}`. With `memory` on, every idea and whether its epoch promoted a
//! candidate is kept in `$TINYRSI_HOME/retreat/<loop>/ideas.json`, and later
//! retreats are told to move away from ideas already tried.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tinybackrooms::{Conversant, Engine, GenerateRequest};
use tinyrsi_gateway::Gateway;
use tinyrsi_model::{ModelConversant, OpenRouterModel, block_on};

use crate::{
    Error, Result,
    engine::Ideator,
    manifest::{self, Loop},
    store::{self, Store},
};

/// Prior ideas shown to a retreat as material to move away from.
const PRIOR_IDEAS: usize = 20;
/// Token cap for one retreat, across every call.
const RETREAT_TOKENS: u64 = 2_000_000;

/// One remembered idea.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdeaRecord {
    /// The idea as shown in the brief.
    pub idea: String,
    /// Whether its epoch promoted a candidate; `None` until known.
    pub promoted: Option<bool>,
}

/// The retreat for one run.
pub struct Retreat {
    engine: Engine,
    config: manifest::Retreat,
    memory: Option<PathBuf>,
    pending: Vec<String>,
}

impl std::fmt::Debug for Retreat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Retreat")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Retreat {
    /// The retreat a loop configures, calling models through the gateway, or
    /// `None` when it has none or it is disabled.
    ///
    /// # Errors
    /// Returns [`Error::Proposer`] when the model backend cannot be built.
    pub fn new(parsed: &Loop, store: &Store, gateway: &Gateway) -> Result<Option<Self>> {
        let Some(config) = parsed.manifest.creativity.retreat.clone().filter(|r| r.enabled) else {
            return Ok(None);
        };
        let port = OpenRouterModel::via(
            parsed.retreat_model().to_owned(),
            &gateway.tagged_url("retreat", "backrooms"),
        )
        .map_err(|e| Error::Proposer(format!("retreat model: {e}")))?;
        let backend = ModelConversant::new(Arc::new(port), 0, RETREAT_TOKENS, 32_768, 4_096, 180_000)
            .map_err(|e| Error::Proposer(format!("retreat backend: {e}")))?;
        let memory = config
            .memory
            .then(|| store.retreat_dir(&parsed.manifest.name).join("ideas.json"));
        Ok(Some(Self::with_backend(Arc::new(backend), config, memory)))
    }

    /// A retreat over any backend; tests pass a scripted one.
    #[must_use]
    pub fn with_backend(
        backend: Arc<dyn Conversant>,
        config: manifest::Retreat,
        memory: Option<PathBuf>,
    ) -> Self {
        Self {
            engine: Engine::new(backend),
            config,
            memory,
            pending: Vec::new(),
        }
    }

    fn remembered(&self) -> Vec<IdeaRecord> {
        self.memory
            .as_deref()
            .and_then(|path| store::read_json(path).ok())
            .unwrap_or_default()
    }

    /// The tinybackrooms request for one epoch.
    fn request(&self, epoch: u32, brief: &str) -> Result<GenerateRequest> {
        let seeds: Vec<Value> = self
            .remembered()
            .iter()
            .rev()
            .take(PRIOR_IDEAS)
            .map(|r| {
                let outcome = match r.promoted {
                    Some(true) => " (its epoch improved the incumbent)",
                    Some(false) => " (its epoch did not improve the incumbent)",
                    None => "",
                };
                json!({ "kind": "prior_idea", "text": format!("{}{outcome}", r.idea) })
            })
            .collect();
        let per_variation = self.config.ideas.div_ceil(self.config.variations).max(1);
        let mut config = json!({
            "variations": self.config.variations,
            "generations": self.config.turns,
            "items_per_variation": per_variation,
            "seed": epoch,
        });
        for (key, value) in [("mode", &self.config.mode), ("template", &self.config.template)] {
            if let Some(value) = value {
                config[key] = json!(value);
            }
        }
        if let Some(t) = self.config.temperature {
            config["temperature"] = json!(t);
        }
        serde_json::from_value(json!({
            "input": { "brief": brief, "seeds": seeds },
            "output": {
                "item_name": "improvement idea",
                "item_guidance": "One concrete, general change worth trying next, and why it might help.",
                "schema": {
                    "type": "object",
                    "required": ["idea", "why"],
                    "properties": {
                        "idea": { "type": "string" },
                        "why": { "type": "string" }
                    }
                }
            },
            "config": config
        }))
        .map_err(|e| Error::Proposer(format!("retreat request: {e}")))
    }
}

impl Ideator for Retreat {
    fn ideas(&mut self, epoch: u32, brief: &str, dir: &Path) -> Result<Vec<String>> {
        let request = self.request(epoch, brief)?;
        let response = block_on(self.engine.generate(&request))
            .map_err(|e| Error::Proposer(format!("retreat: {e}")))?;
        store::write_json(&dir.join("retreat.json"), &response)?;
        let ideas: Vec<String> = response
            .items
            .iter()
            .take(self.config.ideas as usize)
            .filter_map(|item| {
                let idea = item.value.get("idea")?.as_str()?;
                let why = item.value.get("why").and_then(Value::as_str).unwrap_or("");
                Some(if why.is_empty() {
                    idea.to_owned()
                } else {
                    format!("{idea} — {why}")
                })
            })
            .collect();
        self.pending.clone_from(&ideas);
        Ok(ideas)
    }

    fn outcome(&mut self, _epoch: u32, promoted: bool) -> Result<()> {
        let Some(path) = self.memory.clone() else {
            self.pending.clear();
            return Ok(());
        };
        let mut records = self.remembered();
        records.extend(self.pending.drain(..).map(|idea| IdeaRecord {
            idea,
            promoted: Some(promoted),
        }));
        store::write_json(&path, &records)
    }
}

#[cfg(test)]
mod test;
