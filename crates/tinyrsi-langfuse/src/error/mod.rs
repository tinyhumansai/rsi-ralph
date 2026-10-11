//! Crate-wide error and result types.
//!
//! Messages never echo keys, tokens or payloads.

/// Errors returned by this crate.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The Langfuse client could not be configured: a missing or empty
    /// `LANGFUSE_*` variable or URL, or no async runtime for the transport.
    #[error("invalid langfuse export configuration")]
    InvalidConfig,
    /// Langfuse refused or did not receive an OTLP request.
    #[error("langfuse export failed")]
    ExportFailed,
}

/// The crate's standard result type.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
