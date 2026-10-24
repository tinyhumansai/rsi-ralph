//! The gateway's configuration and its spend snapshot.

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::price::PriceTable;
use crate::request::Limits;

/// The address a gateway listens on unless told otherwise: loopback, with
/// the port picked by the operating system.
pub const DEFAULT_LISTEN: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 0);

/// The upstream a gateway forwards to unless told otherwise.
pub const DEFAULT_UPSTREAM: &str = "https://openrouter.ai/api/v1";

/// Everything [`crate::Gateway::start`] needs.
///
/// Build one with [`GatewayConfig::new`], which fills in the defaults, and
/// override fields as needed. `Debug` output never shows the key.
#[derive(Clone)]
pub struct GatewayConfig {
    /// Listen address; must be loopback. Default `127.0.0.1:0`.
    pub listen: SocketAddr,
    /// Upstream base URL. Default `https://openrouter.ai/api/v1`.
    pub upstream: String,
    /// The real provider key, supplied by the caller. The gateway sends it
    /// upstream and nowhere else: never to clients, files, logs or errors.
    pub api_key: String,
    /// This run's directory: `budget.json`, `meter.jsonl` and, with
    /// [`GatewayConfig::capture`], `captures/`.
    pub run_dir: PathBuf,
    /// The campaign-wide ledger shared by every run. A sibling
    /// `<name>.lock` file marks it as owned while the gateway runs.
    pub total_ledger: PathBuf,
    /// Per-run dollar cap. Must match the limit of an existing run ledger.
    pub run_limit_usd: f64,
    /// Campaign-wide dollar cap. Must match the limit of an existing total
    /// ledger.
    pub total_limit_usd: f64,
    /// Write every forwarded request body under `<run_dir>/captures/`.
    pub capture: bool,
    /// The model allowlist and price ceilings. Default
    /// [`PriceTable::standard`].
    pub prices: PriceTable,
    /// Completion-length and reasoning-effort limits. Default
    /// [`Limits::default`].
    pub limits: Limits,
}

impl GatewayConfig {
    /// A configuration with the default listen address, upstream, price
    /// table and limits, and capture off.
    #[must_use]
    pub fn new(
        api_key: impl Into<String>,
        run_dir: impl Into<PathBuf>,
        total_ledger: impl Into<PathBuf>,
        run_limit_usd: f64,
        total_limit_usd: f64,
    ) -> Self {
        Self {
            listen: DEFAULT_LISTEN,
            upstream: DEFAULT_UPSTREAM.to_owned(),
            api_key: api_key.into(),
            run_dir: run_dir.into(),
            total_ledger: total_ledger.into(),
            run_limit_usd,
            total_limit_usd,
            capture: false,
            prices: PriceTable::standard(),
            limits: Limits::default(),
        }
    }
}

impl std::fmt::Debug for GatewayConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayConfig")
            .field("listen", &self.listen)
            .field("upstream", &self.upstream)
            .field("api_key", &"[redacted]")
            .field("run_dir", &self.run_dir)
            .field("total_ledger", &self.total_ledger)
            .field("run_limit_usd", &self.run_limit_usd)
            .field("total_limit_usd", &self.total_limit_usd)
            .field("capture", &self.capture)
            .field("prices", &self.prices)
            .field("limits", &self.limits)
            .finish()
    }
}

/// What a gateway has charged, read in-process.
///
/// Charged dollars count settled costs, reservations kept because the cost
/// was unknown, and reservations for calls still in flight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spend {
    /// Dollars charged against the run ledger.
    pub run_usd: f64,
    /// Dollars charged against the total ledger, by every run so far.
    pub total_usd: f64,
    /// The run ledger's cap.
    pub run_limit_usd: f64,
    /// The total ledger's cap.
    pub total_limit_usd: f64,
}

impl Spend {
    /// The dollars a new call may still reserve: the smaller of what is left
    /// in the run and total ledgers, never negative.
    #[must_use]
    pub fn remaining_usd(&self) -> f64 {
        let run = self.run_limit_usd - self.run_usd;
        let total = self.total_limit_usd - self.total_usd;
        run.min(total).max(0.0)
    }
}
