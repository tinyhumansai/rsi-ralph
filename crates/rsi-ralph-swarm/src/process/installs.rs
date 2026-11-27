//! Where a host's process harnesses are installed, read from the environment.
//!
//! Each harness is a program the host installs itself; nothing here
//! downloads one. A variable names each program, and each program must
//! report the version tinyhivemind's `harnesses.lock` pins, checked when the
//! seat starts. Process harnesses run on Unix hosts only; elsewhere a seat
//! on one is refused before anything starts, whatever is installed.

use std::path::PathBuf;

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

/// Where a host's process harnesses are installed. A harness whose program
/// is missing here is not installed, and a seat on it is refused before
/// anything starts; in-process seats need none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HarnessInstalls {
    /// `node`, which runs every harness but `OpenHuman`.
    pub node: Option<PathBuf>,
    /// `openhuman-core`.
    pub openhuman: Option<PathBuf>,
    /// `claude-agent-acp`'s `dist/index.js`.
    pub claude_code: Option<PathBuf>,
    /// `codex-acp`'s `dist/index.js`.
    pub codex_acp: Option<PathBuf>,
    /// The Codex binary; a Codex seat needs it and `codex_acp` both.
    pub codex: Option<PathBuf>,
    /// `OpenClaw`'s `openclaw.mjs`.
    pub openclaw: Option<PathBuf>,
}

impl HarnessInstalls {
    /// The installs as tinyhivemind starts them, each pinned to the version
    /// it must report.
    #[cfg(unix)]
    pub(crate) fn pinned(&self) -> tinyhivemind_deploy::Installs {
        use tinyhivemind_deploy::{CodexInstall, Installs, Pinned};
        let pin = |path: &Option<PathBuf>, version| {
            path.as_ref().map(|path| Pinned::new(path.clone(), version))
        };
        let codex = self
            .codex_acp
            .clone()
            .zip(self.codex.clone())
            .map(|(adapter, binary)| CodexInstall {
                adapter: Pinned::new(adapter, CODEX_ACP_VERSION),
                binary,
            });
        Installs {
            node: self.node.clone(),
            openhuman: pin(&self.openhuman, OPENHUMAN_VERSION),
            claude_code: pin(&self.claude_code, CLAUDE_CODE_VERSION),
            codex,
            openclaw: pin(&self.openclaw, OPENCLAW_VERSION),
        }
    }
}

/// The harnesses `lookup` (an environment) names. A variable that is unset
/// or empty names nothing.
pub fn harness_installs(lookup: &dyn Fn(&str) -> Option<String>) -> HarnessInstalls {
    let path = |name: &str| lookup(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    HarnessInstalls {
        node: path(NODE_ENV),
        openhuman: path(OPENHUMAN_ENV),
        claude_code: path(CLAUDE_CODE_ENV),
        codex_acp: path(CODEX_ACP_ENV),
        codex: path(CODEX_ENV),
        openclaw: path(OPENCLAW_ENV),
    }
}
