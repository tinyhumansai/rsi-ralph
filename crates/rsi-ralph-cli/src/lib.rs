//! The `rsi-ralph` binary: improvement loops defined as folders of files.
//!
//! A loop is a folder holding `rsi.json`, a Markdown brief, a tinyhivemind
//! hive package that writes candidates, and small score and gate scripts in
//! any language. This crate parses that folder ([`manifest`]), runs epochs
//! against it, stores every run under `$TINYRSI_HOME`, and serves the
//! visualizer over the store. The contract is
//! [`docs/specs/rsi-cli.md`](https://github.com/tinyhumansai/rsi-ralph/blob/main/docs/specs/rsi-cli.md).
//!
//! # Example
//!
//! ```
//! use std::collections::BTreeMap;
//!
//! // `rsi.json` alone is not a loop: the brief and hive are missing.
//! let mut files = BTreeMap::new();
//! files.insert("rsi.json".to_owned(), "{}".to_owned());
//! assert!(rsi_ralph_cli::manifest::parse(&files).is_err());
//! ```
//!
//! # What this crate does not hold
//!
//! Scoring statistics, the trace format, the process runner and the hive
//! language live in their own crates and are reused here unchanged, so the
//! binary and the library agree on every stored format.

pub mod brief;
pub mod cli;
pub mod doctor;
pub mod engine;
mod error;
pub mod gate;
pub mod history;
pub mod hive;
pub mod init;
pub mod keys;
pub mod manifest;
pub mod retreat;
pub mod score;
pub mod serve;
pub mod step;
pub mod store;
pub mod subject;
pub mod upgrades;

pub use error::{Error, Result, chain as error_chain};
