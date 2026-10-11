//! Crate-wide error and result types.
//!
//! Every fallible public function in this crate returns [`Result`]. The
//! variants are deliberately coarse: a caller decides whether to retry or stop,
//! and either way the message must never echo a prompt, a reply or a
//! credential.

/// Errors returned by this crate.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A request, limit or reply violates the declared bounds: an empty
    /// prompt, a reservation above its cap, an invalid tool, or a reply that
    /// overspent or did not match the requested tool.
    #[error("invalid model request, limit or reply")]
    InvalidRequest,
    /// The provider call failed: missing credentials, transport, timeout,
    /// non-success status, or an undecodable response.
    #[error("model call failed")]
    CallFailed,
}

/// The crate's standard result type.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
