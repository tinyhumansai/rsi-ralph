//! Seat memory: one engine for every seat, bound per seat or shared.
//!
//! Without a [`SwarmMemory`] seats carry no memory (the default). With one,
//! every seat's turns run `OpenHuman`'s memory lifecycle against its engine:
//! a recall pack before each turn, the turn logged after it, and the
//! `memory` tool. The layout below the root is `OpenHuman`'s:
//!
//! ```text
//! <root>                 shared learnings, read by every seat
//! └── agent:<memory id>  one memory agent's conversations
//! ```
//!
//! By default each seat's memory id is its seat id, so a hive of fifty seats
//! keeps fifty conversation namespaces and a seat recalls only its own turns.
//! [`SwarmMemory::shared`] binds every seat to the one id
//! [`SHARED_MEMORY_AGENT`] instead: all seats log to, and recall from, a
//! single namespace, so each sees what every other seat did.

use std::sync::Arc;

use openhuman_embed::MemoryBinding;
use openhuman_embed::memory::api::MemoryEngine;
use tinymemory_integrations::{EngineCredential, EngineSettings};

use crate::{Error, Result};

/// The memory id every seat is bound to when memory is shared.
pub const SHARED_MEMORY_AGENT: &str = "hive";

/// Where a swarm's seats keep their memory; see the module docs.
#[derive(Clone)]
pub struct SwarmMemory {
    engine: Arc<dyn MemoryEngine>,
    root: String,
    shared: bool,
}

impl std::fmt::Debug for SwarmMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwarmMemory")
            .field("engine", &self.engine.descriptor().id)
            .field("root", &self.root)
            .field("shared", &self.shared)
            .finish()
    }
}

impl SwarmMemory {
    /// Per-seat memory under `root` (such as `team:my-loop`) in `engine`.
    ///
    /// # Errors
    /// [`Error::Adapter`] when `root` is not a memory root `OpenHuman`
    /// accepts, or names its default root (blank or `root`), which every
    /// unbound agent shares.
    pub fn new(engine: Arc<dyn MemoryEngine>, root: impl Into<String>) -> Result<Self> {
        let root = tinyhivemind_openhuman::HiveMemory::with_root(root)?
            .root()
            .to_owned();
        Ok(Self {
            engine,
            root,
            shared: false,
        })
    }

    /// Per-seat memory under `root` in the `CortexDB` server at `endpoint`,
    /// authenticated with `key`.
    ///
    /// # Errors
    /// [`Error::MemoryEngine`] when the engine cannot be built (a blank
    /// endpoint or key), and the errors of [`SwarmMemory::new`].
    pub fn cortexdb(endpoint: &str, key: &str, root: impl Into<String>) -> Result<Self> {
        let settings = EngineSettings {
            endpoint: Some(endpoint.to_owned()),
            ..EngineSettings::default()
        };
        let engine = tinymemory_integrations::build_engine(
            tinymemory_integrations::CORTEXDB_ENGINE_ID,
            &settings,
            EngineCredential::Static(key.to_owned()),
        )
        .map_err(|e| Error::MemoryEngine(e.to_string()))?;
        Self::new(engine, root)
    }

    /// Bind every seat to the one memory id [`SHARED_MEMORY_AGENT`] when
    /// `shared` is true; per-seat ids otherwise.
    #[must_use]
    pub fn shared(mut self, shared: bool) -> Self {
        self.shared = shared;
        self
    }

    /// The layout root every seat's memory sits below.
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    /// Whether every seat shares one memory id.
    #[must_use]
    pub fn is_shared(&self) -> bool {
        self.shared
    }

    /// The memory id `seat`'s turns are logged under.
    #[must_use]
    pub fn agent_id<'a>(&self, seat: &'a str) -> &'a str {
        if self.shared {
            SHARED_MEMORY_AGENT
        } else {
            seat
        }
    }

    /// The namespace `seat`'s conversations live in, as announced in
    /// [`crate::SeatInfo::memory`].
    #[must_use]
    pub fn namespace(&self, seat: &str) -> String {
        format!("{}/agent:{}", self.root, self.agent_id(seat))
    }

    /// `seat`'s `OpenHuman` memory binding.
    pub(crate) fn binding(&self, seat: &str) -> MemoryBinding {
        MemoryBinding::new(self.agent_id(seat)).root(self.root.clone())
    }

    /// The engine every seat's memory runs on.
    pub(crate) fn engine(&self) -> Arc<dyn MemoryEngine> {
        self.engine.clone()
    }
}

#[cfg(test)]
mod test;
