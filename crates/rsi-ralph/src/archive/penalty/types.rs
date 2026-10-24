//! Strikes against patch shapes and the cooldown policy that reads them.
use serde::{Deserialize, Serialize};
use tinyhivemind_lang::patch::PatchClass;

/// A patch's class and the targets it edits, ignoring literal values.
///
/// Two patches that edit the same targets with different text share a shape,
/// so strikes catch near-duplicates that exact deduplication misses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchShape {
    /// The patch's authority class.
    pub class: PatchClass,
    /// Sorted, deduplicated `(operation tag, target)` pairs: the JSON pointer,
    /// the document or prompt path, `path#item` for a context item and
    /// `identity#entry` for a learning operation.
    pub targets: Vec<(String, String)>,
}

/// Why a patch shape earned a strike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrikeKind {
    /// The candidate failed runs its incumbent completed (`HARMFUL`).
    Harmful,
    /// A `NO_GAIN` or `NOT_REPLICATED` stop whose normalized upper bound was
    /// below zero: a confident regression, not merely a flat result.
    Regressed,
    /// The host rolled back a deployed acceptance of this shape.
    Revoked,
}

/// One piece of evidence that a patch shape made the hive worse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strike {
    /// The struck shape.
    pub shape: PatchShape,
    /// Host epoch of the stop or revocation.
    pub epoch: u64,
    /// What the evidence was.
    pub kind: StrikeKind,
    /// Index of the stop in `Archive::proposals`, absent for revocations.
    pub proposal: Option<usize>,
}

/// Every strike in archive order; a read-only projection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PenaltyReport {
    /// Strikes in the order their events were recorded.
    pub strikes: Vec<Strike>,
}

/// How many strikes, within how many epochs, cool a patch shape.
///
/// A `Harmful` or `Revoked` strike in the window cools a shape on its own;
/// otherwise `strikes` of them must fall in the window. Zero `strikes` or
/// zero `window_epochs` disables cooldown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PenaltyPolicy {
    /// Strikes in the window that cool a shape.
    pub strikes: u32,
    /// Epochs a strike stays in the window, counting its own epoch.
    pub window_epochs: u64,
}

impl Default for PenaltyPolicy {
    /// Two strikes within three epochs.
    fn default() -> Self {
        Self {
            strikes: 2,
            window_epochs: 3,
        }
    }
}
