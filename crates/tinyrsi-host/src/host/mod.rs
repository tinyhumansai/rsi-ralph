//! Durable host activation and observation using the real hive coordinator.
//!
//! `DeploymentStore` keeps what is actually deployed separate from what the
//! archive has selected. It checkpoints redacted coordinator observations per
//! epoch (`observe`), activates the accepted archive head (`activate`),
//! restores its parent (`rollback`) and finishes or compensates an
//! interrupted transition on restart (`recover`). `CoordinatorHost` is the
//! `ActivationBackend` that rebuilds a real coordinator. See this directory's
//! `README.md` for the host lifecycle and the quiescence requirement.
mod runtime;
mod types;
use crate::{Archive, Error, Observation, Observer, Result};
pub use runtime::CoordinatorHost;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tinyhivemind_hives::Coordinator;
use tinyhivemind_lang::Package;
use types::Snapshot;
pub use types::{
    ActivationBackend, DeploymentCheckpoint, DeploymentIntent, DeploymentKind, HostFuture,
    MemorySnapshot, RunnerFactory,
};

#[derive(Debug)]
struct WriterLock(std::fs::File);
impl Drop for WriterLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}
/// Single-writer durable deployment journal.
///
/// The host must stop external admission during all activation operations.
/// A pending intent always blocks new activation until recovery succeeds.
#[derive(Debug)]
pub struct DeploymentStore {
    path: PathBuf,
    state: DeploymentCheckpoint,
    _lock: WriterLock,
}
impl DeploymentStore {
    /// Open a checkpoint, validating identities and exclusive ownership.
    /// # Errors
    /// Returns archive I/O, corruption or locking errors.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let input = path.as_ref();
        let parent = input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|_| Error::ArchiveIo)?;
        let path = fs::canonicalize(parent)
            .map_err(|_| Error::ArchiveIo)?
            .join(input.file_name().ok_or(Error::ArchiveIo)?);
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::ArchiveIo);
        }
        let lockpath = sidecar(&path, "lock");
        if fs::symlink_metadata(&lockpath).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::ArchiveIo);
        }
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lockpath)
            .map_err(|_| Error::ArchiveIo)?;
        fs2::FileExt::try_lock_exclusive(&file).map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                Error::ArchiveLocked
            } else {
                Error::ArchiveIo
            }
        })?;
        let lock = WriterLock(file);
        // An interrupted write cannot become history. Exclusive ownership allows
        // discarding its temporary file; the last renamed checkpoint remains authoritative.
        match fs::remove_file(sidecar(&path, "next")) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::ArchiveIo),
        }
        let state = match fs::read(&path) {
            Ok(bytes) => {
                let snapshot: Snapshot =
                    serde_json::from_slice(&bytes).map_err(|_| Error::ArchiveCorrupt)?;
                if snapshot.version != 1 {
                    return Err(Error::ArchiveCorrupt);
                }
                validate(&snapshot.checkpoint)?;
                snapshot.checkpoint
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => DeploymentCheckpoint::default(),
            Err(_) => return Err(Error::ArchiveIo),
        };
        Ok(Self {
            path,
            state,
            _lock: lock,
        })
    }
    /// Current durable state; selection and deployment identities are distinct.
    #[must_use]
    pub const fn checkpoint(&self) -> &DeploymentCheckpoint {
        &self.state
    }
    fn save(&mut self, state: DeploymentCheckpoint) -> Result<()> {
        validate(&state)?;
        let bytes = serde_json::to_vec(&Snapshot {
            version: 1,
            checkpoint: state.clone(),
        })
        .map_err(|_| Error::ArchiveIo)?;
        let temporary = sidecar(&self.path, "next");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| Error::ArchiveIo)?;
        let result = (|| {
            file.write_all(&bytes).map_err(|_| Error::ArchiveIo)?;
            file.sync_all().map_err(|_| Error::ArchiveIo)?;
            fs::rename(&temporary, &self.path).map_err(|_| Error::ArchiveIo)?;
            // std cannot open directories portably; Unix supplies directory fsync.
            #[cfg(unix)]
            std::fs::File::open(self.path.parent().ok_or(Error::ArchiveIo)?)
                .and_then(|f| f.sync_all())
                .map_err(|_| Error::ArchiveIo)?;
            Ok(())
        })();
        let _ = fs::remove_file(temporary);
        // After rename the persisted state may have changed even if directory
        // fsync failed. Fail closed in memory instead of admitting another apply.
        if result.is_ok() || fs::read(&self.path).is_ok_and(|disk| disk == bytes) {
            self.state = state;
        }
        result
    }
    /// Checkpoint one redacted epoch, filtering previously consumed references.
    /// # Errors
    /// Returns observation or persistence errors; failed writes do not consume evidence.
    pub fn observe(&mut self, coordinator: &Coordinator) -> Result<Observation> {
        if self.state.pending.is_some() {
            return Err(Error::HostFailed);
        }
        let mut observer = Observer::new();
        let mut observation = observer.snapshot(coordinator)?;
        observation.transcript.retain(|r| {
            self.state
                .transcript_cursor
                .is_none_or(|cursor| r.sequence > cursor)
        });
        observation
            .episodes
            .retain(|e| !self.state.settled_episodes.contains(&e.episode_id));
        let mut next = self.state.clone();
        next.epoch = next.epoch.checked_add(1).ok_or(Error::HostFailed)?;
        next.transcript_cursor = observer.cursor().or(next.transcript_cursor);
        next.observations.insert(next.epoch, observation.clone());
        for episode in &observation.episodes {
            if matches!(
                episode.phase,
                crate::ObservedEpisodePhase::Settled | crate::ObservedEpisodePhase::Failed
            ) {
                next.settled_episodes.insert(episode.episode_id.clone());
            }
        }
        self.save(next)?;
        Ok(observation)
    }
    /// Activate the exact accepted head, retaining a recoverable intent on failure.
    /// # Errors
    /// Returns an error for pending work, invalid archive identities or host/storage failure.
    pub async fn activate(
        &mut self,
        archive: &Archive,
        backend: &mut impl ActivationBackend,
    ) -> Result<()> {
        let target = archive.head().ok_or(Error::InvalidRecord)?.id.clone();
        if self.state.pending.is_some() {
            return Err(Error::HostFailed);
        }
        if self.state.deployed_id.as_ref() == Some(&target) {
            return Ok(());
        }
        self.transition(archive, backend, target, DeploymentKind::Activate)
            .await
    }
    async fn transition(
        &mut self,
        archive: &Archive,
        backend: &mut impl ActivationBackend,
        target: String,
        kind: DeploymentKind,
    ) -> Result<()> {
        backend.quiescent().await?;
        let (package, watermarks) = accepted(archive, &target)?;
        if let Some(from) = &self.state.deployed_id {
            accepted(archive, from)?;
        }
        let mut next = self.state.clone();
        next.pending = Some(DeploymentIntent {
            from: next.deployed_id.clone(),
            to: target.clone(),
            kind,
            applied: false,
        });
        self.save(next)?;
        backend.apply(package, watermarks).await?;
        let mut next = self.state.clone();
        next.pending.as_mut().ok_or(Error::HostFailed)?.applied = true;
        self.save(next)?;
        if kind == DeploymentKind::Activate {
            let mut next = self.state.clone();
            next.deployed_id = Some(target);
            next.pending = None;
            self.save(next)?;
        }
        Ok(())
    }
    /// Restore the deployed design's accepted parent and then revert archive selection.
    ///
    /// The archive records the rollback as a revocation at the checkpoint's
    /// epoch: the record stays accepted and its patch shape takes a strike.
    /// # Errors
    /// Returns an error if selection diverged, no parent exists, or deployment fails.
    pub async fn rollback(
        &mut self,
        archive: &mut Archive,
        backend: &mut impl ActivationBackend,
    ) -> Result<()> {
        if self.state.pending.is_some() {
            return Err(Error::HostFailed);
        }
        let head = archive.head().ok_or(Error::NoParent)?;
        if self.state.deployed_id.as_ref() != Some(&head.id) {
            return Err(Error::HostFailed);
        }
        let parent = head.parent.clone().ok_or(Error::NoParent)?;
        self.transition(archive, backend, parent.clone(), DeploymentKind::Rollback)
            .await?;
        archive.revoke(self.state.epoch)?;
        let mut next = self.state.clone();
        next.deployed_id = Some(parent);
        next.pending = None;
        self.save(next)
    }
    /// Recover pending deployment, or reattach the durable deployed runtime on restart.
    ///
    /// Completed rollback selection is finished idempotently. Other unfinished
    /// operations restore the prior deployment; first startup retries its target.
    /// # Errors
    /// Missing archive snapshots or failed compensation leave admission closed.
    pub async fn recover(
        &mut self,
        archive: &Archive,
        backend: &mut impl ActivationBackend,
    ) -> Result<()> {
        if let Some(intent) = self.state.pending.clone() {
            backend.quiescent().await?;
            let rollback_done = intent.kind == DeploymentKind::Rollback
                && intent.applied
                && archive.head().is_some_and(|h| h.id == intent.to);
            let restore = if rollback_done || intent.from.is_none() {
                Some(intent.to)
            } else {
                intent.from
            };
            if let Some(id) = &restore {
                let (package, watermarks) = accepted(archive, id)?;
                backend.restore(package, watermarks).await?;
            }
            let mut next = self.state.clone();
            next.deployed_id = restore;
            next.pending = None;
            self.save(next)
        } else if let Some(id) = &self.state.deployed_id {
            let (package, watermarks) = accepted(archive, id)?;
            backend.restore(package, watermarks).await
        } else {
            Ok(())
        }
    }
}
fn accepted<'a>(
    archive: &'a Archive,
    id: &str,
) -> Result<(&'a Package, &'a BTreeMap<String, u64>)> {
    let record = archive
        .records()
        .iter()
        .find(|r| r.id == id && r.verdict == tinyhivemind_lang::lineage::Verdict::Accepted)
        .ok_or(Error::InvalidRecord)?;
    let package = archive.package(id).ok_or(Error::InvalidRecord)?;
    if tinyhivemind_lang::lineage::content_id(package, &record.memory_watermarks)
        .map_err(|_| Error::InvalidRecord)?
        != id
    {
        return Err(Error::InvalidRecord);
    }
    Ok((package, &record.memory_watermarks))
}
fn validate(state: &DeploymentCheckpoint) -> Result<()> {
    let valid = |id: &str| {
        id.len() == 71
            && id.starts_with("sha256:")
            && id[7..]
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    };
    if state
        .observations
        .keys()
        .any(|epoch| *epoch == 0 || *epoch > state.epoch)
    {
        return Err(Error::ArchiveCorrupt);
    }
    if state.deployed_id.as_ref().is_some_and(|id| !valid(id))
        || state.settled_episodes.iter().any(|id| id.trim().is_empty())
    {
        return Err(Error::ArchiveCorrupt);
    }
    if let Some(intent) = &state.pending
        && (!valid(&intent.to)
            || intent.from != state.deployed_id
            || intent.from.as_ref().is_some_and(|id| !valid(id))
            || intent.from.as_ref() == Some(&intent.to))
    {
        return Err(Error::ArchiveCorrupt);
    }
    Ok(())
}
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".{suffix}"));
    PathBuf::from(name)
}

#[cfg(test)]
mod test;
