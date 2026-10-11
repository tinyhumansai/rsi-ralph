//! Exclusive durable candidate history, proposal evidence and active ancestry.
mod normalized;
mod replay;
mod types;
#[cfg(test)]
use replay::validate_watermarks;

use crate::{Error, Result};
use std::collections::BTreeMap;
#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tinyhivemind_lang::Package;
use tinyhivemind_lang::lineage::{self, Record, Verdict, VersionMetadata};
pub use types::{
    EvaluationEvent, NormalizedApprovalEvent, NormalizedDeclarationEvent,
    NormalizedEvaluationEvent, ProposalDeclaration, ProposalEvent,
};
use types::{Event, Snapshot};

#[derive(Debug)]
struct WriterLock(PathBuf);
impl Drop for WriterLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[derive(Clone, Debug, Default)]
struct State {
    events: Vec<Event>,
    records: Vec<Record>,
    packages: BTreeMap<String, Package>,
    prepared: BTreeMap<String, (Package, BTreeMap<String, u64>)>,
    active: Option<String>,
}

/// A single-writer archive holding exact language records and evaluated packages.
///
/// `path` names a JSON file. Writes replace it atomically after syncing a
/// same-directory temporary file; an adjacent lock excludes concurrent writers.
#[derive(Debug)]
pub struct Archive {
    path: PathBuf,
    state: State,
    _lock: WriterLock,
}

impl Archive {
    /// Open and validate the complete history, or initialize an empty archive.
    ///
    /// # Errors
    /// Returns `ArchiveLocked` for a held lock, `ArchiveCorrupt` for malformed
    /// history, and `ArchiveIo` for filesystem failures.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|_| Error::ArchiveIo)?;
        let parent = fs::canonicalize(parent).map_err(|_| Error::ArchiveIo)?;
        let name = path.file_name().ok_or(Error::ArchiveIo)?;
        let path = parent.join(name);
        if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(Error::ArchiveIo);
        }
        let lock_path = sidecar(&path, "lock");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    Error::ArchiveLocked
                } else {
                    Error::ArchiveIo
                }
            })?;
        let lock = WriterLock(lock_path);
        let state = match fs::read(&path) {
            Ok(bytes) => {
                let snapshot: Snapshot =
                    serde_json::from_slice(&bytes).map_err(|_| Error::ArchiveCorrupt)?;
                if !matches!(snapshot.version, 1 | 2) {
                    return Err(Error::ArchiveCorrupt);
                }
                let mut state = State::default();
                for event in snapshot.events {
                    state.push(event).map_err(|_| Error::ArchiveCorrupt)?;
                }
                state
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(_) => return Err(Error::ArchiveIo),
        };
        Ok(Self {
            path,
            state,
            _lock: lock,
        })
    }

    /// Persist a validated, pinned candidate before any host evaluation.
    ///
    /// Prepared snapshots do not fabricate objective scores or activate a head.
    ///
    /// # Errors
    /// Returns `InvalidRecord` for invalid package, watermarks or duplicate
    /// identity, and `ArchiveIo` for storage failures.
    pub fn prepare(
        &mut self,
        package: Package,
        watermarks: BTreeMap<String, u64>,
    ) -> Result<String> {
        tinyhivemind_lang::validate(&package).map_err(|_| Error::InvalidRecord)?;
        let id = lineage::content_id(&package, &watermarks).map_err(|_| Error::InvalidRecord)?;
        self.transact(Event::Prepared {
            id: id.clone(),
            package: Box::new(package),
            watermarks,
        })?;
        Ok(id)
    }

    /// Construct and persist an evaluated record, including the founding design.
    ///
    /// # Errors
    /// Returns `InvalidRecord` for invalid identity, ancestry or patch, and
    /// `ArchiveIo` when persistence fails.
    pub fn record(&mut self, package: Package, metadata: VersionMetadata) -> Result<String> {
        let record = lineage::record(&package, metadata).map_err(|_| Error::InvalidRecord)?;
        let id = record.id.clone();
        self.append(record, package)?;
        Ok(id)
    }

    /// Persist an evaluated record and its final proposal outcome atomically.
    ///
    /// # Errors
    /// Returns `InvalidRecord` for invalid evidence or ancestry, and `ArchiveIo`
    /// when storage fails. Neither event is visible before replacement commits.
    pub fn conclude(
        &mut self,
        package: Package,
        metadata: VersionMetadata,
        proposal: ProposalEvent,
    ) -> Result<String> {
        let record = lineage::record(&package, metadata).map_err(|_| Error::InvalidRecord)?;
        let id = record.id.clone();
        self.transact_many(vec![
            Event::Candidate {
                record: Box::new(record),
                package: Box::new(package),
            },
            Event::Proposal { proposal },
        ])?;
        Ok(id)
    }

    /// Persist a verified record and its exact candidate package transactionally.
    ///
    /// # Errors
    /// Returns `InvalidRecord` for invalid evidence, duplicates or ancestry, and
    /// `ArchiveIo` when persistence fails.
    pub fn append(&mut self, record: Record, package: Package) -> Result<()> {
        self.transact(Event::Candidate {
            record: Box::new(record),
            package: Box::new(package),
        })
    }

    /// Persist the exact proposer prediction and approval inputs before evaluation.
    ///
    /// Declarations are host-private evidence and must not be sent to retreat
    /// models without the public projection applied by the retreat module.
    ///
    /// # Errors
    /// Returns `InvalidRecord` for nonfinite predictions and `ArchiveIo` when
    /// the declaration cannot be persisted.
    pub fn declare_proposal(&mut self, declaration: ProposalDeclaration) -> Result<()> {
        self.transact(Event::Declaration { declaration })
    }

    /// Exact pre-evaluation proposals in durable insertion order.
    #[must_use]
    pub fn declarations(&self) -> Vec<&ProposalDeclaration> {
        self.state
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Declaration { declaration } => Some(declaration),
                _ => None,
            })
            .collect()
    }

    /// Persist a rejected or guarded attempt even if its patch is invalid.
    ///
    /// # Errors
    /// Returns `ArchiveIo` when the audit event cannot be persisted.
    pub fn record_proposal(&mut self, proposal: ProposalEvent) -> Result<()> {
        self.transact(Event::Proposal { proposal })
    }

    /// Persist exact paired outcomes before or after the candidate record.
    ///
    /// # Errors
    /// Returns `InvalidRecord` for malformed identity or incomplete/nonfinite
    /// paired outcomes, and `ArchiveIo` for storage failures.
    pub fn record_evaluation(&mut self, evaluation: EvaluationEvent) -> Result<()> {
        self.transact(Event::Evaluation { evaluation })
    }

    /// Exact paired evaluation evidence in durable insertion order.
    #[must_use]
    pub fn evaluations(&self) -> Vec<&EvaluationEvent> {
        self.state
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Evaluation { evaluation } => Some(evaluation),
                _ => None,
            })
            .collect()
    }

    /// All evaluated designs in durable insertion order.
    #[must_use]
    pub fn records(&self) -> &[Record] {
        &self.state.records
    }

    /// Exact evaluated package for a content identity.
    #[must_use]
    pub fn package(&self, id: &str) -> Option<&Package> {
        self.state
            .packages
            .get(id)
            .or_else(|| self.state.prepared.get(id).map(|(package, _)| package))
    }

    /// Currently active accepted design, including persisted reversions.
    #[must_use]
    pub fn head(&self) -> Option<&Record> {
        self.state
            .active
            .as_ref()
            .and_then(|id| self.state.records.iter().find(|record| &record.id == id))
    }

    /// Rejected evaluated designs retained as proposal evidence.
    #[must_use]
    pub fn rejected(&self) -> Vec<&Record> {
        self.state
            .records
            .iter()
            .filter(|record| record.verdict == Verdict::Rejected)
            .collect()
    }

    /// Attempts without an evaluated candidate, in durable insertion order.
    #[must_use]
    pub fn proposals(&self) -> Vec<&ProposalEvent> {
        self.state
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Proposal { proposal } => Some(proposal),
                _ => None,
            })
            .collect()
    }

    /// Activate the current accepted design's immediate accepted parent.
    ///
    /// # Errors
    /// Returns `NoParent` if the head has no accepted parent, and `ArchiveIo`
    /// when the activation cannot be persisted.
    pub fn revert(&mut self) -> Result<String> {
        let id = self.state.revert_target()?.to_owned();
        self.transact(Event::Activation { id: id.clone() })?;
        Ok(id)
    }

    fn temporary_path(&self) -> PathBuf {
        sidecar(&self.path, "pending")
    }

    fn transact(&mut self, event: Event) -> Result<()> {
        self.transact_many(vec![event])
    }

    fn transact_many(&mut self, events: Vec<Event>) -> Result<()> {
        let mut next = self.state.clone();
        for event in events {
            next.push(event)?;
        }
        let snapshot = Snapshot {
            version: if next.events.iter().any(|event| {
                matches!(
                    event,
                    Event::NormalizedDeclaration { .. }
                        | Event::NormalizedEvaluation { .. }
                        | Event::NormalizedApproval { .. }
                )
            }) {
                2
            } else {
                1
            },
            events: next.events.clone(),
        };
        let bytes = serde_json::to_vec(&snapshot).map_err(|_| Error::ArchiveIo)?;
        let temporary = self.temporary_path();
        match fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::ArchiveIo),
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| Error::ArchiveIo)?;
        let result = (|| {
            file.write_all(&bytes).map_err(|_| Error::ArchiveIo)?;
            file.sync_all().map_err(|_| Error::ArchiveIo)?;
            fs::rename(&temporary, &self.path).map_err(|_| Error::ArchiveIo)?;
            // The rename has committed: keep memory consistent even if directory
            // sync fails and the caller receives an uncertain durability error.
            self.state = next;
            #[cfg(unix)]
            {
                let parent = self.path.parent().ok_or(Error::ArchiveIo)?;
                File::open(parent)
                    .and_then(|directory| directory.sync_all())
                    .map_err(|_| Error::ArchiveIo)
            }
            // Portable std cannot open directories for syncing on Windows.
            // The file is synced and atomically replaced, but power-loss
            // persistence of the directory entry depends on the filesystem.
            #[cfg(not(unix))]
            {
                Ok(())
            }
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".{suffix}"));
    PathBuf::from(name)
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod test_evidence;

#[cfg(test)]
mod test_float;

#[cfg(test)]
mod test_normalized;
