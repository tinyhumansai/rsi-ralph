//! Immutable verifier declarations and public artifact evidence.
use serde::{Deserialize, Serialize};
/// Host-frozen subprocess specification; arguments are never shell evaluated.
#[derive(Clone, Debug)]
pub struct CommandSpec {
    /// Direct executable path.
    pub program: String,
    /// Arguments; `{artifact}` and `{workspace}` expand to host-staged paths.
    pub args: Vec<String>,
    /// Host-selected workspace root for all source and compiler output.
    pub workspace_root: std::path::PathBuf,
    /// Maximum elapsed milliseconds.
    pub timeout_ms: u64,
    /// Maximum combined stdout and stderr bytes.
    pub max_output_bytes: usize,
}
/// Strong host-owned checker implementations.
#[derive(Clone, Debug)]
pub enum HostVerifier {
    /// Execute a host-declared checker on the artifact.
    Process(CommandSpec),
    /// Compare exact bytes against a private golden reference.
    Golden {
        /// Private expected artifact bytes.
        expected: Vec<u8>,
    },
    /// Validate a JSON artifact against a host-frozen schema.
    Schema {
        /// Private JSON schema.
        schema: serde_json::Value,
    },
    /// Check a generated proof block in a frozen Lean theorem harness.
    Lean {
        /// Host executable and limits; must invoke the genuine Lean checker.
        command: CommandSpec,
        /// Frozen theorem prelude, ending immediately before the generated proof.
        theorem: String,
    },
}
/// Immutable mapping from DAG node to its authorized checker and seat.
#[derive(Clone, Debug)]
pub struct VerifierDeclaration {
    /// DAG node identity.
    pub node_id: String,
    /// Authorized submitting seat.
    pub owner_seat: String,
    /// Host-only verifier configuration.
    pub verifier: HostVerifier,
}
/// Exact host-authorized episode, seat, node and transcript binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationBinding {
    /// Active coordinator episode.
    pub episode_id: String,
    /// Registered seat identity.
    pub seat_id: String,
    /// Blueprint node identity.
    pub node_id: String,
    /// Host-bound artifact identity.
    pub artifact_id: String,
    /// Actual visible transcript sequence references.
    pub transcript_refs: Vec<u64>,
}
/// Model-produced artifact; its binding must equal the host authorization.
#[derive(Clone, Debug)]
pub struct ArtifactSubmission {
    /// Claimed identity, independently validated against the host binding.
    pub binding: VerificationBinding,
    /// Artifact bytes, retained only in the host verifier workspace.
    pub content: Vec<u8>,
}
/// Public checker evidence without private oracle or process output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationEvidence {
    /// Checked identity.
    pub binding: VerificationBinding,
    /// SHA-256 of the exact submitted artifact bytes.
    pub artifact_digest: String,
    /// Strong checker result.
    pub passed: bool,
    /// Finite verified credit, exactly zero or one.
    pub quality: f64,
    /// Observed subprocess exit code, absent for nonprocess or interrupted checks.
    pub exit_code: Option<i32>,
}
