//! Crate-wide error and result types.
//!
//! The first two variants keep the names and messages the `tinyrsi` crate
//! has always reported, so a caller that matched them before the split still
//! matches the same failures.

/// Errors returned by this crate.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Paired evaluation data, budgets, statistical policy or scorer
    /// configuration are invalid.
    #[error("invalid evaluation data or policy")]
    InvalidEvaluation,
    /// A host-owned trial executor failed.
    #[error("host operation failed")]
    HostFailed,
    /// A judge's reply was not one well-formed grade in range.
    #[error("judge reply is not a well-formed grade")]
    JudgeMalformed,
    /// A judge's model call failed, overspent or was refused.
    #[error(transparent)]
    Model(#[from] tinyrsi_model::Error),
}

/// The crate's standard result type.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
