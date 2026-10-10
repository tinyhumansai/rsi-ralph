//! A `TinyBus` module that holds the lineage of a recursive self-improvement
//! run.
//!
//! The module is the loop's ledger. Every candidate agent configuration the
//! improvement loop produces is graded and recorded here as one
//! [`GradeEntry`]: the candidate's snapshot address, its private grade, the
//! verdict the loop reached, and the rationale for that verdict. The module
//! serves [`append`] over the bus and answers which entry is the current
//! incumbent.
//!
//! # Layout
//!
//! This is the implementation half of a two-crate workspace:
//!
//! - [`tinyrsi_bus`] — the wire contract. Member names, payload types, and the
//!   contract version, with no transport and no behavior. A host that only
//!   makes calls depends on that crate alone.
//! - `tinyrsi` — this crate. The behavior, the crate-wide error type, and the
//!   `TinyBus` adapter that serves them, built as both an `rlib` and the
//!   `cdylib` the loader consumes.
//!
//! Within this crate:
//!
//! - `src/error/` holds the crate-wide [`Error`] enum and the [`Result`] alias
//!   returned by every fallible public function.
//! - Each feature area lives in its own module directory with a `mod.rs`
//!   module root, an optional `types.rs`, and a `test.rs` holding its unit
//!   tests.
//! - Every public item is re-exported from here — including all of
//!   [`tinyrsi_bus`] — so downstream users have a single predictable surface
//!   and `tinyrsi::GradeEntry` is the *same type* as
//!   `tinyrsi_bus::GradeEntry`, not a structural twin.
//! - `tinybus_module` adapts the public behavior to `TinyBus` and exports the
//!   module descriptor, embedded manifest, and initialization entrypoint.
//!
//! # Example
//!
//! ```
//! use tinyrsi::{append, Error, Lineage, LineageAppendRequest};
//! use tinyrsi_bus::Verdict;
//!
//! let mut lineage = Lineage::new();
//! let request = LineageAppendRequest::new(
//!     tinyrsi::GradeEntry::new(0, "sha256:abc", 0.703, Verdict::Accepted, "baseline"),
//! );
//! assert_eq!(append(&mut lineage, request.clone())?, true);
//! assert_eq!(append(&mut lineage, request).unwrap_err(), Error::StepOutOfOrder);
//! # Ok::<(), tinyrsi::Error>(())
//! ```

mod error;
mod lineage;
mod tinybus_module;

pub use error::{Error, Result};
pub use lineage::Lineage;

// The wire contract, re-exported by module rather than by item so every path
// through this crate resolves to the same definitions the contract crate
// publishes. A host may depend on `tinyrsi-bus` directly and get exactly these
// types; nothing here redefines them.
pub use tinyrsi_bus;
pub use tinyrsi_bus::{
    CONTRACT_VERSION, GradeEntry, INTERFACE, LineageAppendRequest, LineageAppendResponse,
    LineageHeadRequest, LineageHeadResponse, METHODS, OBJECT_PATH, Verdict, is_compatible, names,
    version,
};

/// Records one graded candidate in `lineage`.
///
/// The entry is valid when its [`GradeEntry::step`] equals the ledger's current
/// length and its snapshot address and rationale are non-empty; see
/// [`Lineage::append`].
///
/// # Errors
///
/// Returns [`Error::StepOutOfOrder`] when the entry's step does not continue
/// the ledger, and [`Error::EmptyField`] when the snapshot address or the
/// rationale is empty.
pub fn append(lineage: &mut Lineage, request: LineageAppendRequest) -> Result<bool> {
    lineage.append(request.entry)
}

/// Answers which entry is the current incumbent.
///
/// The head is the last accepted entry; with no accepted entries recorded, the
/// response carries `None`.
#[must_use]
pub fn head(lineage: &Lineage) -> LineageHeadResponse {
    LineageHeadResponse::new(lineage.head().cloned())
}
