//! Blueprint configuration and externally observed result states.
use serde::{Deserialize, Serialize};
/// Verifier strength, ordered strongest to weakest.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum VerifierKind {
    /// Machine-checked proof.
    Proof,
    /// Executed tests or other reproducible checks.
    Execution,
    /// Structural schema validation.
    Schema,
    /// Comparison to a host-owned held-out reference.
    HeldOutReference,
    /// Judge applying a scoring rubric; weak evidence.
    Rubric,
    /// Preference judgment; weak evidence.
    Preference,
}
impl VerifierKind {
    /// Whether this verifier relies on weak judge or preference evidence.
    #[must_use]
    pub const fn is_weak(self) -> bool {
        matches!(self, Self::Rubric | Self::Preference)
    }
}
/// Immutable verifier configuration owned by the host.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Verifier {
    /// Strength and verification mechanism.
    pub kind: VerifierKind,
    /// Host-defined specification or reference identifier.
    pub spec: String,
}
/// Result state of a blueprint node.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum NodeStatus {
    /// Work has not yet passed verification.
    Open,
    /// A known incomplete step.
    Sorry,
    /// Host verifier confirmed the statement.
    Verified,
    /// Host verifier rejected the statement.
    Failed,
}
/// One statement, its responsible seat role, and its verification contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BlueprintNode {
    /// Unique nonempty graph identity.
    pub id: String,
    /// Statement to establish.
    pub statement: String,
    /// Responsible seat role.
    pub owner_role: String,
    /// Node identities that must first be verified.
    pub dependencies: Vec<String>,
    /// Fixed host verifier definition.
    pub verifier: Verifier,
    /// Initial unverified state; subsequent state is owned by the graph.
    pub status: NodeStatus,
}
