//! Crate-wide error and result types.

/// Errors returned by this crate.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A sink could not record an event: the trace file already existed,
    /// could not be created, or a line could not be encoded or written.
    #[error("loop trace write failed")]
    Io,
    /// An exporter was configured without a usable endpoint or header.
    #[error("invalid trace exporter configuration")]
    InvalidExporter,
}

/// The crate's standard result type.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
