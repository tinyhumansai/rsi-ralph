//! Durable immutable hive-memory snapshots, shadow learning and exact restoration.
//!
//! A watermark names immutable contents in a declared namespace. Learning
//! prepares a new snapshot without changing active memory; activation and revert
//! select existing snapshots. Run-local writes are returned as owned maps.
use crate::{CandidateState, Error, Operation, Package, Patch, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tinyhivemind_lang::MemoryLifecycle;

type Entries = BTreeMap<String, String>;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Image {
    namespace: String,
    watermark: u64,
    entries: Entries,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    images: BTreeMap<String, Image>,
    active: BTreeMap<String, u64>,
    namespaces: BTreeMap<String, String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            images: BTreeMap::new(),
            active: BTreeMap::new(),
            namespaces: BTreeMap::new(),
        }
    }
}
struct Lock(File);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}
/// File-backed immutable snapshots shared by evaluation and deployment.
#[derive(Clone)]
pub struct SnapshotMemoryStore {
    path: PathBuf,
    lock: PathBuf,
    forbidden: Vec<String>,
    max_bytes: usize,
}
impl std::fmt::Debug for SnapshotMemoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnapshotMemoryStore")
            .field("path", &self.path)
            .field("max_bytes", &self.max_bytes)
            .finish_non_exhaustive()
    }
}
impl SnapshotMemoryStore {
    /// Open an atomically persisted snapshot store with contamination canaries.
    /// # Errors
    /// Rejects invalid bounds/canaries, corrupt data, symlinks and storage errors.
    pub fn open(path: impl AsRef<Path>, forbidden: Vec<String>, max_bytes: usize) -> Result<Self> {
        if max_bytes == 0 || forbidden.iter().any(String::is_empty) {
            return Err(Error::InvalidRecord);
        }
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|_| Error::ArchiveIo)?;
        let parent = fs::canonicalize(parent).map_err(|_| Error::ArchiveIo)?;
        let path = parent.join(path.file_name().ok_or(Error::ArchiveIo)?);
        let lock = path.with_file_name(format!(
            "{}.lock",
            path.file_name().ok_or(Error::ArchiveIo)?.to_string_lossy()
        ));
        let this = Self {
            path,
            lock,
            forbidden,
            max_bytes,
        };
        this.transaction(false, |_| Ok(()))?;
        Ok(this)
    }
    /// Seed host-owned founding snapshots; an existing watermark cannot change.
    /// # Errors
    /// Rejects missing/extra identities, conflicting contents and contamination.
    pub fn seed(
        &self,
        package: &Package,
        marks: &BTreeMap<String, u64>,
        values: BTreeMap<String, Entries>,
    ) -> Result<()> {
        let namespaces = namespaces(package, marks)?;
        if values.keys().ne(marks.keys()) {
            return Err(Error::InvalidRecord);
        }
        self.transaction(true, |state| {
            for (id, entries) in values {
                self.insert(
                    state,
                    Image {
                        namespace: namespaces[&id].clone(),
                        watermark: marks[&id],
                        entries,
                    },
                )?;
            }
            Ok(())
        })
    }
    /// Read an owned, isolated map from exact immutable snapshots.
    /// # Errors
    /// Rejects missing snapshots and invalid namespace/watermark declarations.
    pub fn read(
        &self,
        package: &Package,
        marks: &BTreeMap<String, u64>,
    ) -> Result<BTreeMap<String, Entries>> {
        let namespaces = namespaces(package, marks)?;
        self.transaction(false, |state| {
            namespaces
                .into_iter()
                .map(|(id, namespace)| {
                    let image = state
                        .images
                        .get(&image_key(&namespace, marks[&id]))
                        .ok_or(Error::InvalidRecord)?;
                    Ok((id, image.entries.clone()))
                })
                .collect()
        })
    }
    /// Read immutable snapshots from memory already validated by language lowering.
    ///
    /// The host runner (`tinyrsi-host`) uses this to preserve seat bindings
    /// without reconstructing an incomplete package. The runner separately
    /// enforces each seat's recall grants.
    /// # Errors
    /// Rejects invalid identities, watermarks, namespaces and missing snapshots.
    pub fn read_spec(
        &self,
        memory: &tinyhivemind_lang::MemorySpec,
        marks: &BTreeMap<String, u64>,
    ) -> Result<BTreeMap<String, Entries>> {
        let mut package = Package {
            memory: memory.clone(),
            ..Package::default()
        };
        package.memory.bindings.clear();
        self.read(&package, marks)
    }
    /// Prepare checked learning snapshots without modifying live memory.
    /// # Errors
    /// Rejects stale entries/watermarks, readonly targets, contamination, overflow
    /// and conflicting snapshots. Caller separately enforces the language guard.
    pub fn pin(&self, incumbent: &CandidateState, patch: &Patch) -> Result<BTreeMap<String, u64>> {
        let source_namespaces = namespaces(&incumbent.package, &incumbent.memory_watermarks)?;
        self.transaction(true, |state| {
            let mut marks = incumbent.memory_watermarks.clone();
            let mut changed: BTreeMap<String, Entries> = BTreeMap::new();
            for operation in &patch.operations {
                if let Operation::Learning {
                    identity,
                    entry,
                    watermark,
                    before,
                    after,
                } = operation
                {
                    let spec = incumbent
                        .package
                        .memory
                        .identities
                        .iter()
                        .find(|i| &i.id == identity)
                        .ok_or(Error::InvalidRecord)?;
                    if patch.class != crate::PatchClass::Memory
                        || spec.read_only
                        || spec.lifecycle != MemoryLifecycle::Hive
                        || entry.trim().is_empty()
                        || marks.get(identity) != Some(watermark)
                    {
                        return Err(Error::InvalidRecord);
                    }
                    let namespace = source_namespaces
                        .get(identity)
                        .ok_or(Error::InvalidRecord)?;
                    let base = state
                        .images
                        .get(&image_key(namespace, *watermark))
                        .ok_or(Error::InvalidRecord)?;
                    let entries = changed
                        .entry(identity.clone())
                        .or_insert_with(|| base.entries.clone());
                    if entries.get(entry) != before.as_ref() {
                        return Err(Error::InvalidRecord);
                    }
                    if let Some(value) = after {
                        self.clean(value)?;
                        entries.insert(entry.clone(), value.clone());
                    } else {
                        entries.remove(entry);
                    }
                }
            }
            for (id, entries) in changed {
                let next = marks[&id].checked_add(1).ok_or(Error::InvalidRecord)?;
                self.insert(
                    state,
                    Image {
                        namespace: source_namespaces[&id].clone(),
                        watermark: next,
                        entries,
                    },
                )?;
                marks.insert(id, next);
            }
            let candidate = tinyhivemind_lang::patch::apply(&incumbent.package, patch)
                .map_err(|_| Error::InvalidRecord)?
                .candidate;
            self.reconcile(
                state,
                &candidate,
                &source_namespaces,
                &incumbent.memory_watermarks,
                &mut marks,
            )?;
            Ok(marks)
        })
    }
    fn reconcile(
        &self,
        state: &mut State,
        candidate: &Package,
        source_namespaces: &BTreeMap<String, String>,
        original_marks: &BTreeMap<String, u64>,
        marks: &mut BTreeMap<String, u64>,
    ) -> Result<()> {
        let persistent: BTreeMap<_, _> = candidate
            .memory
            .identities
            .iter()
            .filter(|i| i.lifecycle == MemoryLifecycle::Hive)
            .map(|i| (i.id.clone(), i))
            .collect();
        marks.retain(|id, _| persistent.contains_key(id));
        for (id, spec) in &persistent {
            if let Some(old_namespace) = source_namespaces.get(id) {
                if old_namespace != &spec.namespace {
                    return Err(Error::InvalidRecord);
                }
            } else {
                let entries = if let Some(parent) = &spec.fork_parent {
                    let parent_namespace =
                        source_namespaces.get(parent).ok_or(Error::InvalidRecord)?;
                    let mark = original_marks.get(parent).ok_or(Error::InvalidRecord)?;
                    state
                        .images
                        .get(&image_key(parent_namespace, *mark))
                        .ok_or(Error::InvalidRecord)?
                        .entries
                        .clone()
                } else {
                    BTreeMap::new()
                };
                self.insert(
                    state,
                    Image {
                        namespace: spec.namespace.clone(),
                        watermark: 0,
                        entries,
                    },
                )?;
                marks.insert(id.clone(), 0);
            }
        }
        let expected = namespaces(candidate, marks)?;
        for (id, namespace) in expected {
            if !state
                .images
                .contains_key(&image_key(&namespace, marks[&id]))
            {
                return Err(Error::InvalidRecord);
            }
        }
        Ok(())
    }
    /// Atomically select validated existing snapshots for the live host.
    /// # Errors
    /// Missing snapshots leave the previous live selection untouched.
    pub fn activate(&self, package: &Package, marks: &BTreeMap<String, u64>) -> Result<()> {
        let namespaces = namespaces(package, marks)?;
        self.transaction(true, |state| {
            for (id, namespace) in &namespaces {
                if !state.images.contains_key(&image_key(namespace, marks[id])) {
                    return Err(Error::InvalidRecord);
                }
            }
            state.active = marks.clone();
            state.namespaces = namespaces;
            Ok(())
        })
    }
    /// Return the currently deployed watermark selection without memory text.
    /// # Errors
    /// Returns storage/locking errors or corrupt persisted state.
    pub fn active_watermarks(&self) -> Result<BTreeMap<String, u64>> {
        self.transaction(false, |state| Ok(state.active.clone()))
    }
    fn clean(&self, text: &str) -> Result<()> {
        if text.len() > self.max_bytes || self.forbidden.iter().any(|f| text.contains(f)) {
            Err(Error::InvalidRecord)
        } else {
            Ok(())
        }
    }
    fn insert(&self, state: &mut State, image: Image) -> Result<()> {
        for (key, value) in &image.entries {
            if key.trim().is_empty() {
                return Err(Error::InvalidRecord);
            }
            self.clean(key)?;
            self.clean(value)?;
        }
        let key = image_key(&image.namespace, image.watermark);
        if let Some(old) = state.images.get(&key) {
            if old.entries != image.entries {
                return Err(Error::InvalidRecord);
            }
        } else {
            state.images.insert(key, image);
        }
        Ok(())
    }
    fn transaction<T>(
        &self,
        write: bool,
        apply: impl FnOnce(&mut State) -> Result<T>,
    ) -> Result<T> {
        for path in [&self.path, &self.lock] {
            if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(Error::ArchiveIo);
            }
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock)
            .map_err(|_| Error::ArchiveIo)?;
        if write {
            FileExt::lock_exclusive(&file)
        } else {
            FileExt::lock_shared(&file)
        }
        .map_err(|_| Error::ArchiveLocked)?;
        let _guard = Lock(file);
        let mut state = match fs::read(&self.path) {
            Ok(bytes) if bytes.len() <= self.max_bytes => {
                serde_json::from_slice::<State>(&bytes).map_err(|_| Error::ArchiveCorrupt)?
            }
            Ok(_) => return Err(Error::ArchiveCorrupt),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(_) => return Err(Error::ArchiveIo),
        };
        if state.version != 1 || state.active.keys().ne(state.namespaces.keys()) {
            return Err(Error::ArchiveCorrupt);
        }
        for (key, image) in &state.images {
            if key != &image_key(&image.namespace, image.watermark)
                || image.namespace.is_empty()
                || image
                    .entries
                    .iter()
                    .any(|(k, v)| self.clean(k).is_err() || self.clean(v).is_err())
            {
                return Err(Error::ArchiveCorrupt);
            }
        }
        for (id, mark) in &state.active {
            if !state
                .images
                .contains_key(&image_key(&state.namespaces[id], *mark))
            {
                return Err(Error::ArchiveCorrupt);
            }
        }
        let result = apply(&mut state)?;
        if write {
            let bytes = serde_json::to_vec(&state).map_err(|_| Error::ArchiveIo)?;
            if bytes.len() > self.max_bytes {
                return Err(Error::InvalidRecord);
            }
            let parent = self.path.parent().ok_or(Error::ArchiveIo)?;
            let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|_| Error::ArchiveIo)?;
            temp.write_all(&bytes)
                .and_then(|()| temp.as_file().sync_all())
                .map_err(|_| Error::ArchiveIo)?;
            temp.persist(&self.path).map_err(|_| Error::ArchiveIo)?;
            #[cfg(unix)]
            File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(|_| Error::ArchiveIo)?;
        }
        Ok(result)
    }
}
fn namespaces(
    package: &Package,
    marks: &BTreeMap<String, u64>,
) -> Result<BTreeMap<String, String>> {
    tinyhivemind_lang::validate(package).map_err(|_| Error::InvalidRecord)?;
    tinyhivemind_lang::lineage::content_id(package, marks).map_err(|_| Error::InvalidRecord)?;
    Ok(package
        .memory
        .identities
        .iter()
        .filter(|i| i.lifecycle == MemoryLifecycle::Hive)
        .map(|i| (i.id.clone(), i.namespace.clone()))
        .collect())
}
fn image_key(namespace: &str, mark: u64) -> String {
    let mut hash = Sha256::new();
    hash.update(namespace.as_bytes());
    hash.update([0]);
    hash.update(mark.to_be_bytes());
    format!("{:x}", hash.finalize())
}
#[cfg(test)]
mod test;
