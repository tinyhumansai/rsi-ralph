//! The hive host runtime: everything that needs a live coordinator.
//!
//! `tinyrsi` decides which hive design is better. This crate runs designs:
//!
//! - [`CoordinatorHost`] builds tinyhivemind coordinators over SQLite storage,
//!   with runners from a [`RunnerFactory`].
//! - [`DeploymentStore`] activates, rolls back and recovers deployments.
//! - [`CoordinatorEvaluator`] evaluates candidates on frozen private suites
//!   ([`PrivateSuites`]) in isolated coordinator episodes.
//! - [`EvidenceJournal`] durably records measured turns and checker evidence.
//! - `prepare_retreat_epoch` / `evaluate_retreat_epoch` drive one durable
//!   improvement epoch through an [`EpochJournal`].
//!
//! It re-exports all of `tinyrsi`, so a host imports from one crate. It
//! replaces the old `tinyrsi` `host` Cargo feature: depend on `tinyrsi-host`
//! and import `tinyrsi_host::Name` where you imported `tinyrsi::Name` with
//! that feature enabled.
//!
//! Errors are `tinyrsi`'s own [`Error`]: the host runtime has always reported
//! that crate's variants, and it still does.
//!
//! # Example
//!
//! ```
//! use tinyrsi_host::{Archive, MemoryLimits};
//!
//! let limits = MemoryLimits::default();
//! assert!(limits.max_entries > 0);
//! let dir = tempfile::tempdir()?;
//! let archive = Archive::open(dir.path().join("archive.json"))?;
//! assert!(archive.head().is_none());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # What this crate does not hold
//!
//! No decision rules and no proposal generation: scoring, evolution and the
//! selection funnel stay in `tinyrsi` and the crates below it.

mod epoch_driver;
mod evidence_journal;
mod host;
mod suites;

pub use epoch_driver::*;
pub use evidence_journal::{EvidenceEpoch, EvidenceJournal, RecordedTurn, RecordedTurnStatus};
pub use host::*;
pub use suites::*;
pub use tinyrsi::*;

/// A snapshot store restores the pinned memory images a deployment records.
impl MemorySnapshot for SnapshotMemoryStore {
    fn restore<'a>(
        &'a self,
        package: &'a Package,
        marks: &'a std::collections::BTreeMap<String, u64>,
    ) -> HostFuture<'a, ()> {
        Box::pin(async move { self.activate(package, marks) })
    }
}
