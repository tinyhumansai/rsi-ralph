//! The improvement lineage: the ledger of graded candidates the RSI loop
//! produces.
//!
//! A module root like this one documents the module, wires its pieces
//! together, and exposes the smallest useful API. Substantial type definitions
//! belong in a sibling `types.rs` (here, the contract crate), and unit tests
//! belong in `test.rs`, wired in at the bottom of this file.
//!
//! [`Lineage`] holds [`GradeEntry`] records in step order. Recording requires
//! each entry's [`GradeEntry::step`] to continue the ledger exactly, and every
//! entry to name its snapshot and carry a rationale; the head is the last
//! accepted entry, which is the incumbent the loop improves next.

use crate::{Error, Result};
use tinyrsi_bus::GradeEntry;

/// The lineage ledger.
///
/// Entries are kept in the order they were recorded; because recording pins
/// each step to the ledger's length, that order is the step order.
#[derive(Debug, Default, Clone)]
pub struct Lineage {
    entries: Vec<GradeEntry>,
}

impl Lineage {
    /// Creates an empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `entry` if its step continues the ledger.
    ///
    /// An entry is valid when its [`GradeEntry::step`] equals the ledger's
    /// current length, its [`GradeEntry::snapshot`] is non-empty, and its
    /// [`GradeEntry::rationale`] is non-empty. A valid entry that names an
    /// existing step replaces nothing: it is rejected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::StepOutOfOrder`] when the entry's step does not equal
    /// the ledger's length, and [`Error::EmptyField`] when the snapshot
    /// address or the rationale is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tinyrsi::{Lineage, Result};
    /// # use tinyrsi_bus::{GradeEntry, Verdict};
    /// # fn run() -> Result<()> {
    /// let mut lineage = Lineage::new();
    /// lineage.append(GradeEntry::new(0, "sha256:abc", 0.703, Verdict::Accepted, "baseline"))?;
    /// assert_eq!(lineage.len(), 1);
    /// # Ok::<(), tinyrsi::Error>(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn append(&mut self, entry: GradeEntry) -> Result<bool> {
        if entry.snapshot.trim().is_empty() || entry.rationale.trim().is_empty() {
            return Err(Error::EmptyField);
        }
        if entry.step != self.entries.len() as u64 {
            return Err(Error::StepOutOfOrder);
        }
        self.entries.push(entry);
        Ok(true)
    }

    /// The current incumbent: the last accepted entry, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tinyrsi::Lineage;
    /// assert!(Lineage::new().head().is_none());
    /// ```
    #[must_use]
    pub fn head(&self) -> Option<&GradeEntry> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.verdict == tinyrsi_bus::Verdict::Accepted)
    }

    /// Every recorded entry, in step order.
    #[must_use]
    pub fn entries(&self) -> &[GradeEntry] {
        &self.entries
    }

    /// How many entries the ledger holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the ledger holds no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod test;
