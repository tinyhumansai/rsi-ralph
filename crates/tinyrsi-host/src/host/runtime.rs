//! Concrete rebuilding coordinator adapter.
use super::{ActivationBackend, HostFuture, MemorySnapshot, RunnerFactory};
use crate::{Error, Result};
use std::{collections::BTreeMap, path::Path, sync::Arc};
use tinyhivemind_hives::{
    AgentRegistration, Coordinator, CoordinatorOptions, DeliveryStatus, HiveInfo, Storage,
};
use tinyhivemind_lang::{LoweredHive, Package};
/// Concrete coordinator backend rebuilding supplied configured runners.
///
/// Removed seats remain dormant durable identities. Existing hive metadata may
/// not change because the upstream coordinator has no metadata mutation API.
pub struct CoordinatorHost {
    runtime_id: String,
    storage: Arc<dyn Storage>,
    factory: Arc<dyn RunnerFactory>,
    memory: Arc<dyn MemorySnapshot>,
    coordinator: Option<Arc<Coordinator>>,
}
impl std::fmt::Debug for CoordinatorHost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoordinatorHost")
            .field("runtime_id", &self.runtime_id)
            .field("active", &self.coordinator.is_some())
            .finish_non_exhaustive()
    }
}
impl CoordinatorHost {
    /// Construct a backend without claiming storage ownership yet.
    #[must_use]
    pub fn new(
        runtime_id: String,
        storage: Arc<dyn Storage>,
        factory: Arc<dyn RunnerFactory>,
        memory: Arc<dyn MemorySnapshot>,
    ) -> Self {
        Self {
            runtime_id,
            storage,
            factory,
            memory,
            coordinator: None,
        }
    }
    /// Open the upstream SQLite store for process-safe coordinator restart.
    /// # Errors
    /// Returns an error if SQLite cannot open or validate its state.
    pub fn sqlite(
        runtime_id: String,
        path: impl AsRef<Path>,
        factory: Arc<dyn RunnerFactory>,
        memory: Arc<dyn MemorySnapshot>,
    ) -> Result<Self> {
        let storage =
            tinyhivemind_hives::SqliteStorage::open(path).map_err(|_| Error::HostFailed)?;
        Ok(Self::new(runtime_id, Arc::new(storage), factory, memory))
    }
    /// Live coordinator after successful activation or recovery.
    #[must_use]
    pub fn coordinator(&self) -> Option<&Coordinator> {
        self.coordinator.as_deref()
    }
    async fn rebuild(
        &mut self,
        package: &Package,
        watermarks: &BTreeMap<String, u64>,
        recovering: bool,
    ) -> Result<()> {
        if !recovering || self.coordinator.is_some() {
            self.quiescent().await?;
        }
        let hive = tinyhivemind_lang::lower(package).map_err(|_| Error::InvalidRecord)?;
        self.factory.validate(&hive)?;
        let stored = self.storage.load().await.map_err(|_| Error::HostFailed)?;
        for desk in &hive.desks {
            if let Some(existing) = stored.hives.get(&desk.id)
                && (existing.name != desk.name || existing.description != desk.description)
            {
                return Err(Error::HostFailed);
            }
        }
        let runners = hive
            .seats
            .iter()
            .map(|seat| {
                let session = stored
                    .agents
                    .get(&seat.id)
                    .and_then(|a| a.session_id.as_deref());
                self.factory
                    .build(&hive, seat, session)
                    .map(|runner| (seat.id.clone(), session.map(str::to_owned), runner))
            })
            .collect::<Result<Vec<_>>>()?;
        self.memory.restore(package, watermarks).await?;
        let coordinator = Coordinator::new(
            self.runtime_id.clone(),
            self.storage.clone(),
            options(&hive),
        )
        .await
        .map_err(|_| Error::HostFailed)?;
        // Publish the newly fenced runtime before later registration errors;
        // callers must recover the durable intent instead of using old handles.
        let coordinator = Arc::new(coordinator);
        self.coordinator = Some(coordinator.clone());
        self.factory.attach(&coordinator)?;
        for (agent_id, session, runner) in runners {
            let registration = AgentRegistration {
                agent_id,
                runtime_id: self.runtime_id.clone(),
                runner,
            };
            if let Some(session) = session {
                coordinator
                    .register_agent_in_session(registration, &session)
                    .await
            } else {
                coordinator.register_agent(registration).await
            }
            .map_err(|_| Error::HostFailed)?;
        }
        for existing in coordinator.list_hives().map_err(|_| Error::HostFailed)? {
            let wanted = hive.desks.iter().find(|d| d.id == existing.hive_id);
            for member in &existing.members {
                if wanted.is_none_or(|d| d.members != existing.members) {
                    coordinator
                        .leave_hive(&existing.hive_id, member)
                        .await
                        .map_err(|_| Error::HostFailed)?;
                }
            }
        }
        for desk in &hive.desks {
            if stored.hives.contains_key(&desk.id) {
                let current = coordinator
                    .list_hives()
                    .map_err(|_| Error::HostFailed)?
                    .into_iter()
                    .find(|h| h.hive_id == desk.id)
                    .ok_or(Error::HostFailed)?;
                for member in &desk.members {
                    if !current.members.contains(member) {
                        coordinator
                            .join_hive(&desk.id, member)
                            .await
                            .map_err(|_| Error::HostFailed)?;
                    }
                }
            } else {
                coordinator
                    .create_hive(HiveInfo {
                        hive_id: desk.id.clone(),
                        name: desk.name.clone(),
                        description: desk.description.clone(),
                        members: desk.members.clone(),
                    })
                    .await
                    .map_err(|_| Error::HostFailed)?;
            }
        }
        Ok(())
    }
}
impl ActivationBackend for CoordinatorHost {
    fn quiescent(&self) -> HostFuture<'_, ()> {
        Box::pin(async {
            let state = self.storage.load().await.map_err(|_| Error::HostFailed)?;
            if !state.running.is_empty()
                || state.agents.values().any(|a| a.parked)
                || state
                    .deliveries
                    .iter()
                    .any(|d| matches!(d.status, DeliveryStatus::Pending | DeliveryStatus::Running))
                || state.episodes.iter().any(|e| !e.finished)
            {
                return Err(Error::HostFailed);
            }
            Ok(())
        })
    }
    fn apply<'a>(
        &'a mut self,
        package: &'a Package,
        watermarks: &'a BTreeMap<String, u64>,
    ) -> HostFuture<'a, ()> {
        Box::pin(self.rebuild(package, watermarks, false))
    }
    fn restore<'a>(
        &'a mut self,
        package: &'a Package,
        watermarks: &'a BTreeMap<String, u64>,
    ) -> HostFuture<'a, ()> {
        Box::pin(self.rebuild(package, watermarks, true))
    }
}
fn options(hive: &LoweredHive) -> CoordinatorOptions {
    crate::suites::coordinator_options(hive)
}
