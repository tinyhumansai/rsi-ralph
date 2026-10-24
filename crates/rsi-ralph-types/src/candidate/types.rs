//! Candidate state and the evaluation stages it passes through.
use std::collections::BTreeMap;
use tinyhivemind_lang::Package;

/// Cheap-to-expensive evaluation stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalStage {
    /// Small replay and regression screen.
    Screen,
    /// Complete selection suite.
    Selection,
    /// Fresh private tasks and seeds.
    Confirmation,
}

/// Candidate spec plus the exact pinned hive memory evaluated with it.
#[derive(Debug, Clone)]
pub struct CandidateState {
    /// Immutable candidate package.
    pub package: Package,
    /// Persistent memory event watermarks, complete for the package.
    pub memory_watermarks: BTreeMap<String, u64>,
}
