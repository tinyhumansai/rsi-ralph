//! Preregistered bounded original-versus-evolved orchestrator experiments.
//!
//! `ExperimentProtocol` freezes both orchestrator configurations, the initial
//! hive, private suite fingerprints, seeds, caps and the reporting rule before
//! any model output exists. `run_experiment` asks the host's
//! `ExperimentExecutor` for every Full, `MemoryOff` and `RetreatOff` trial,
//! rechecks the returned evidence against the protocol and reports a verdict.
//! See this directory's `README.md` for the endpoint and its limits.

mod ledger;
mod protocol;
mod report;
mod types;
pub use report::run_experiment;
pub use types::*;

#[cfg(test)]
mod test;
