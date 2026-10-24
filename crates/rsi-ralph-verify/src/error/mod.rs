//! Crate-wide error and result types.
//!
//! The variants keep the names and messages the `tinyrsi` crate has always
//! reported, so a caller that matched them before the split still matches.

/// Errors returned by this crate.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A blueprint has invalid nodes, dependencies or verification order, or
    /// a verifier declaration is inconsistent.
    #[error("invalid blueprint or verification outcome")]
    InvalidBlueprint,
    /// A host-owned process, file or checker operation failed.
    #[error("host operation failed")]
    HostFailed,
}

/// The crate's standard result type.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
