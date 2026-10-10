//! The bus identity of the tinyrsi module: interface name, object path, and
//! one constant per member.
//!
//! Nothing here is a string literal at a call site. A host names a member
//! through [`methods`] and the object through [`OBJECT_PATH`], so a rename is a
//! compile error in every consumer rather than a runtime "unknown method".
//!
//! The interface, the path, and the member constants rename together, and
//! [`METHODS`] stays in the same order as the interface's dispatch table.

/// The well-known interface name the module claims on the bus.
pub const INTERFACE: &str = "ai.tinyhumans.tinyrsi.Lineage";

/// The object path the module serves its interface at.
pub const OBJECT_PATH: &str = "/ai/tinyhumans/tinyrsi/Lineage";

/// One constant per member of [`INTERFACE`].
pub mod methods {
    /// Records one graded candidate in the lineage.
    ///
    /// Takes a [`crate::LineageAppendRequest`] and returns a
    /// [`crate::LineageAppendResponse`].
    pub const APPEND: &str = "Append";

    /// Answers which entry is the current incumbent.
    ///
    /// Takes a [`crate::LineageHeadRequest`] and returns a
    /// [`crate::LineageHeadResponse`].
    pub const HEAD: &str = "Head";
}

/// Every member of [`INTERFACE`], in the order the interface dispatches them.
///
/// `crates/tinyrsi` asserts its declared manifest methods against this list,
/// so the two cannot drift.
pub const METHODS: &[&str] = &[methods::APPEND, methods::HEAD];

#[cfg(test)]
mod test;
