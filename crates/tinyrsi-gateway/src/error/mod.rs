//! The crate-wide error type.
//!
//! No variant ever carries the provider key: messages hold paths, addresses,
//! dollar amounts and upstream text from which the key has been redacted.

use std::net::SocketAddr;
use std::path::PathBuf;

/// Everything the gateway can refuse or fail on.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A ledger, meter or lock file could not be read or written.
    #[error("cannot access {path}: {source}")]
    Io {
        /// The file that failed.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A ledger file exists but is not a valid ledger.
    #[error("ledger {path} is corrupt: {message}")]
    LedgerFormat {
        /// The ledger file.
        path: PathBuf,
        /// What was wrong with it.
        message: String,
    },
    /// An existing ledger was reopened with a different limit.
    #[error("ledger {path} has limit ${stored}, refusing to reopen it with ${requested}")]
    LimitChanged {
        /// The ledger file.
        path: PathBuf,
        /// The limit stored in the file.
        stored: f64,
        /// The limit the caller asked for.
        requested: f64,
    },
    /// A limit is zero, negative or not finite.
    #[error("spend limit must be a positive finite number, got {0}")]
    InvalidLimit(f64),
    /// A call's worst-case cost does not fit in a ledger's remaining budget.
    #[error("{ledger} budget exhausted: call may cost ${needed:.6}, ${remaining:.6} remains")]
    BudgetExceeded {
        /// `run` or `total`.
        ledger: &'static str,
        /// The reservation the call needed.
        needed: f64,
        /// What the ledger had left.
        remaining: f64,
    },
    /// A call cost more than its reservation; all spending stops.
    #[error("a call exceeded its reserved price bound; spending is halted")]
    PriceBoundBreached,
    /// The requested model is not on the allowlist.
    #[error("model {0:?} is not allowed through this gateway")]
    UnknownModel(String),
    /// The request body is not something the gateway forwards.
    #[error("bad request: {0}")]
    BadRequest(String),
    /// The upstream provider could not be reached.
    #[error("upstream request failed: {0}")]
    Upstream(String),
    /// Another gateway already holds the total ledger.
    #[error("{0} exists: another gateway owns this ledger (remove the file if it is stale)")]
    Locked(PathBuf),
    /// The listen address is not loopback; the gateway holds a real key and
    /// must never be reachable from another machine.
    #[error("gateway must listen on a loopback address, got {0}")]
    NonLoopback(SocketAddr),
    /// The listen address could not be bound.
    #[error("cannot listen on {addr}: {source}")]
    Bind {
        /// The address that failed.
        addr: SocketAddr,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The background thread or its Tokio runtime could not be started.
    #[error("cannot start the gateway runtime: {0}")]
    Runtime(#[source] std::io::Error),
    /// The HTTP server failed while serving.
    #[error("gateway server failed: {0}")]
    Serve(#[source] std::io::Error),
    /// The background thread exited without reporting (it panicked).
    #[error("gateway thread stopped unexpectedly")]
    ServerStopped,
}

/// Result alias used by every fallible public function in this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
