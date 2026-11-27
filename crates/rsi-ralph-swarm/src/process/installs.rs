//! Where a host's process harnesses are installed, read from the environment.
//!
//! Each harness is a program the host installs itself; nothing here
//! downloads one. A variable names each program, and each program must
//! report the version tinyhivemind's `harnesses.lock` pins, checked when the
//! seat starts.

use std::path::PathBuf;

use tinyhivemind_deploy::{CodexInstall, Installs, Pinned};

/// `node`, which runs Claude Code's, Codex's and `OpenClaw`'s ACP programs.
pub const NODE_ENV: &str = "TINYRSI_NODE";
/// `openhuman-core`, for seats on the `openhuman` harness.
pub const OPENHUMAN_ENV: &str = "TINYRSI_OPENHUMAN_CORE";
/// `@agentclientprotocol/claude-agent-acp`'s `dist/index.js`, for seats on
/// `claude-code`.
pub const CLAUDE_CODE_ENV: &str = "TINYRSI_CLAUDE_CODE_ACP";
/// `@agentclientprotocol/codex-acp`'s `dist/index.js`, for seats on `codex`.
pub const CODEX_ACP_ENV: &str = "TINYRSI_CODEX_ACP";
/// The Codex binary `codex-acp` runs, which also writes each seat's model
/// catalog.
pub const CODEX_ENV: &str = "TINYRSI_CODEX";
/// `OpenClaw`'s `openclaw.mjs`, for seats on `openclaw`.
pub const OPENCLAW_ENV: &str = "TINYRSI_OPENCLAW";

/// The `openhuman-core` version the pinned tinyhivemind is tested against.
pub const OPENHUMAN_VERSION: &str = "0.64.15";
/// The `claude-agent-acp` version.
pub const CLAUDE_CODE_VERSION: &str = "0.112.0";
/// The `codex-acp` version.
pub const CODEX_ACP_VERSION: &str = "2.12.1";
/// The `OpenClaw` version.
pub const OPENCLAW_VERSION: &str = "2026.10.33";

/// The harnesses `lookup` (an environment) names. A harness whose variable
/// is unset or empty is not installed; a seat on it is refused before
/// anything starts.
pub fn harness_installs(lookup: &dyn Fn(&str) -> Option<String>) -> Installs {
    let path = |name: &str| lookup(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let codex = path(CODEX_ACP_ENV)
        .zip(path(CODEX_ENV))
        .map(|(adapter, binary)| CodexInstall {
            adapter: Pinned::new(adapter, CODEX_ACP_VERSION),
            binary,
        });
    Installs {
        node: path(NODE_ENV),
        openhuman: path(OPENHUMAN_ENV).map(|p| Pinned::new(p, OPENHUMAN_VERSION)),
        claude_code: path(CLAUDE_CODE_ENV).map(|p| Pinned::new(p, CLAUDE_CODE_VERSION)),
        codex,
        openclaw: path(OPENCLAW_ENV).map(|p| Pinned::new(p, OPENCLAW_VERSION)),
    }
}
