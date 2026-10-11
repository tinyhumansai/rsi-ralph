//! Crate-wide error and result types.
//!
//! Manifest errors name the offending field or path so `tinyrsi check` can
//! point at the exact line of `rsi.json` to fix. Runtime errors wrap the
//! crate that failed.

use std::path::PathBuf;

/// Errors returned by this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A file the loop folder must contain is missing.
    #[error("missing file {path}")]
    MissingFile {
        /// Path relative to the loop folder.
        path: String,
    },
    /// `rsi.json` is not valid JSON or does not match the schema.
    #[error("rsi.json: {message}")]
    Json {
        /// Parser message, naming the field when serde can.
        message: String,
    },
    /// `$schema` or `version` is not the supported one.
    #[error("unsupported schema {schema} version {version}")]
    Schema {
        /// The declared schema.
        schema: String,
        /// The declared version.
        version: u32,
    },
    /// A field holds a value outside its allowed range or shape.
    #[error("invalid {field}: {reason}")]
    Field {
        /// Dotted path of the field, such as `score.decide.alpha`.
        field: String,
        /// What is wrong with it.
        reason: String,
    },
    /// A path is absolute, escapes its folder or is otherwise unsafe.
    #[error("unsafe path {path}")]
    Path {
        /// The offending path.
        path: String,
    },
    /// A name refers to something that does not exist.
    #[error("unknown {kind} {id}")]
    Reference {
        /// What kind of thing was referenced, such as `seat`.
        kind: &'static str,
        /// The missing identity.
        id: String,
    },
    /// The hive package is not a valid tinyhivemind package.
    #[error("hive package: {0}")]
    Hive(#[from] tinyhivemind_lang::Error),
    /// Required keys are not set in the environment or `.env`.
    #[error("missing keys: {}", names.join(", "))]
    MissingKeys {
        /// Environment variable names, never values.
        names: Vec<String>,
    },
    /// A filesystem operation failed.
    #[error("{action} {}: {source}", path.display())]
    Io {
        /// What was being done.
        action: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A stored JSON document could not be read or written.
    #[error("{}: {message}", path.display())]
    Store {
        /// The document.
        path: PathBuf,
        /// What went wrong.
        message: String,
    },
    /// A run with this id already exists in the store.
    #[error("run {0} already exists")]
    RunExists(String),
    /// No stored run matches.
    #[error("no run {0}")]
    NoRun(String),
    /// A step command could not be started or produced no usable result.
    #[error("step {step}: {reason}")]
    Step {
        /// Step id, such as `score:accuracy` or `gate:1`.
        step: String,
        /// What went wrong.
        reason: String,
    },
    /// A git command failed.
    #[error("git {args}: {stderr}")]
    Git {
        /// The arguments that were run.
        args: String,
        /// The last line of git's stderr.
        stderr: String,
    },
    /// The proposer (the hive) failed.
    #[error("proposer: {0}")]
    Proposer(String),
    /// Scoring evidence could not be compared.
    #[error(transparent)]
    Score(#[from] tinyrsi_score::Error),
    /// The loop trace could not be written.
    #[error(transparent)]
    Trace(#[from] tinyrsi_trace::Error),
    /// A bounded child process could not be run.
    #[error(transparent)]
    Process(#[from] tinyrsi_verify::Error),
}

impl Error {
    /// An [`Error::Field`] for `field`.
    pub(crate) fn field(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Field {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// An [`Error::Io`] wrapping `source`.
    pub(crate) fn io(
        action: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }
}

/// The crate's standard result type.
pub type Result<T> = std::result::Result<T, Error>;
