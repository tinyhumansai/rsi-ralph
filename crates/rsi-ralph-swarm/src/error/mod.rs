//! Crate-wide error and result types.
//!
//! Messages name paths, seats and capabilities, never a tool secret or a
//! model key: an error may be logged or written into a run report.

use std::path::PathBuf;

/// Errors returned by this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A hive package file or directory could not be read.
    #[error("cannot read hive package path {path}")]
    PackageIo {
        /// The file or directory that failed.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },
    /// A hive package file name or its contents are not UTF-8.
    #[error("hive package path {path} is not utf-8")]
    PackageEncoding {
        /// The offending file.
        path: PathBuf,
    },
    /// The package does not parse, validate or lower in the hive language.
    #[error("invalid hive package")]
    Package(#[from] tinyhivemind_lang::Error),
    /// The requested lead is not a seat of the package.
    #[error("lead {0} is not a seat of the hive package")]
    UnknownLead(String),
    /// An editing seat (`action_dir` = `{candidate}`) has no candidate checkout.
    #[error("seat {0} edits a candidate but no candidate checkout was given")]
    MissingCandidate(String),
    /// A candidate checkout was given for a seat that does not edit one.
    #[error("a candidate checkout was given for {0}, which is not an editing seat")]
    UnexpectedCandidate(String),
    /// A package seat uses an identifier the swarm reserves for itself.
    #[error("seat id {0} is reserved by the swarm")]
    ReservedSeat(String),
    /// A seat's executable tool runs a package file (`tools/…`), which the
    /// swarm cannot run: it is handed the package's text, not its directory.
    #[error("seat {seat} declares tool {tool}, whose program is a package file")]
    PackageProgram {
        /// The seat declaring it.
        seat: String,
        /// The tool's id.
        tool: String,
    },
    /// A seat's executable tool is named like a tool `OpenHuman` registers on
    /// every seat itself, so attaching it would fail the first epoch.
    #[error(
        "seat {seat} declares tool {tool}, but tool name {tool:?} is already registered by the seat runtime; rename it in hive.json"
    )]
    ToolNameClash {
        /// The seat declaring it.
        seat: String,
        /// The tool's id.
        tool: String,
    },
    /// A tool key is scoped to a seat the package does not declare.
    #[error("tool key for {capability} names unknown seat {seat}")]
    UnknownToolSeat {
        /// The key's capability.
        capability: String,
        /// The unknown seat.
        seat: String,
    },
    /// A tool key names a capability or provider the swarm cannot wire.
    #[error("unsupported tool key {capability}/{provider}")]
    UnsupportedTool {
        /// The requested capability.
        capability: String,
        /// The requested provider.
        provider: String,
    },
    /// The incumbent or a candidate checkout does not resolve to a directory.
    #[error("cannot open checkout {path}")]
    Checkout {
        /// The checkout path as given.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },
    /// The run directory could not be created, or a run file not written.
    #[error("cannot write swarm run file {path}")]
    RunDir {
        /// The directory or file that failed.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },
    /// The `OpenHuman` runtime could not be built.
    #[error("cannot start the agent runtime")]
    Runtime(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// `OpenHuman` refused a seat's agent specification.
    #[error("cannot build the agent for seat {seat}")]
    Agent {
        /// The seat whose agent failed.
        seat: String,
        /// The harness's refusal.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// The hive coordinator or its storage failed.
    #[error("hive coordinator failed")]
    Coordinator(#[from] tinyhivemind_hives::Error),
    /// The `OpenHuman` hive adapter refused a seat or setting.
    #[error("hive adapter failed")]
    Adapter(#[from] tinyhivemind_openhuman::Error),
    /// The seats' memory engine could not be built; the message names the
    /// engine and the reason, never its key.
    #[error("cannot build the seat memory engine: {0}")]
    MemoryEngine(String),
    /// The dedicated swarm thread or its async runtime could not start.
    #[error("cannot start the swarm thread")]
    Thread(#[source] std::io::Error),
    /// The swarm thread panicked before returning an outcome.
    #[error("the swarm thread panicked")]
    ThreadPanicked,
}

/// The crate's standard result type.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
