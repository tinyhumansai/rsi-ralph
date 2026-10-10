//! The payloads the `Append` and `Head` members exchange.
//!
//! A module root like this one documents the module, wires its pieces together,
//! and exposes the smallest useful API. The type definitions live in the
//! sibling `types.rs`, and the unit tests in `test.rs`, wired in at the bottom
//! of this file.

mod types;

pub use types::{
    GradeEntry, LineageAppendRequest, LineageAppendResponse, LineageHeadRequest,
    LineageHeadResponse, Verdict,
};

#[cfg(test)]
mod test;
