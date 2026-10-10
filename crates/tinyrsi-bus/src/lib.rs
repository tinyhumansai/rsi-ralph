//! Every type that crosses the tinyrsi module's `TinyBus` boundary, and the
//! names of the members that carry them.
//!
//! This crate ships as a loadable `TinyBus` module: `crates/tinyrsi` is built
//! as a `cdylib` and exports one object. A host that loads that binary can call
//! into it but cannot `use` anything out of it, so the payload vocabulary has
//! to be published as an ordinary library. This is that library.
//!
//! # What is here
//!
//! - [`names`] — the interface name, the object path, and one constant per
//!   member, plus [`names::METHODS`] listing them in dispatch order.
//! - [`lineage`] — the value vocabulary: the payloads the `Append` member
//!   exchanges, carrying one graded candidate of the improvement lineage.
//! - [`version`] — [`CONTRACT_VERSION`] and the [`is_compatible`] bind rule.
//!
//! # What is deliberately not here
//!
//! **No behavior.** The `append` implementation lives in `crates/tinyrsi`,
//! which depends on this crate and re-exports it. A payload type describes what
//! a frame carries, not what the module does with it.
//!
//! **No transport.** This crate does not depend on `tinybus` and holds no
//! connection, client, or codec. A host already owns its connection — its
//! reconnect policy, its timeouts, its tracing — and the useful part is the
//! vocabulary, not another wrapper around it.
//!
//! That is also a structural necessity, not only a preference: `tinybus` is
//! vendored as a submodule whose manifest inherits fields from its own nested
//! `[workspace.package]`. A crate that every workspace member can depend on has
//! to stay transport-free, and staying transport-free is what keeps this crate
//! limited to pure-Rust vocabulary dependencies.
//!
//! # This crate sits underneath the implementation, not beside it
//!
//! `tinyrsi` **depends on this crate and re-exports all of it**, so
//! `tinyrsi::GradeEntry` and `tinyrsi_bus::lineage::GradeEntry` are the *same
//! type*, not structural twins. Defining a parallel set of payload types for
//! hosts would mean a conversion at every call site that nothing checks.
//! One definition, here, at the bottom.
//!
//! So: a module author depends on `tinyrsi` and gets behavior and vocabulary.
//! A host depends on `tinyrsi-bus` and gets vocabulary alone.
//!
//! # Staying in step with the module
//!
//! [`names::METHODS`] lists every member. `crates/tinyrsi` asserts its served
//! members against that list, in order, so a method added to the interface
//! without an entry here fails that crate's tests rather than surfacing as an
//! unknown method in a host at runtime.
//!
//! # Example
//!
//! ```
//! use tinyrsi_bus::{names, GradeEntry, LineageAppendRequest, Verdict};
//!
//! let entry = GradeEntry::new(0, "sha256:abc", 0.703, Verdict::Accepted, "baseline");
//! let body = serde_json::to_value([LineageAppendRequest::new(entry)])?;
//! assert_eq!(names::methods::APPEND, "Append");
//! assert_eq!(names::OBJECT_PATH, "/ai/tinyhumans/tinyrsi/Lineage");
//!
//! let reply = serde_json::from_value::<serde_json::Value>(
//!     serde_json::json!({ "recorded": true }),
//! )?;
//! assert_eq!(reply["recorded"], true);
//! # Ok::<(), serde_json::Error>(())
//! ```

pub mod lineage;
pub mod names;
pub mod version;

pub use lineage::{
    GradeEntry, LineageAppendRequest, LineageAppendResponse, LineageHeadRequest,
    LineageHeadResponse, Verdict,
};
pub use names::{INTERFACE, METHODS, OBJECT_PATH};
pub use version::{CONTRACT_VERSION, is_compatible};

// These are the language's own wire definitions, not parallel payloads.
pub use tinyhivemind_lang::Package;
pub use tinyhivemind_lang::lineage::{Record, VersionMetadata};
pub use tinyhivemind_lang::patch::{Operation, Patch, PatchClass};
