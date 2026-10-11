//! The single spend choke point for a `tinyrsi` run.
//!
//! Every model call a run makes goes through this OpenAI-compatible
//! loopback proxy to the openrouter.ai API, which:
//!
//! 1. allows only the models in its [`PriceTable`];
//! 2. pins each model to one provider with a price ceiling, bounds
//!    `max_tokens`, and applies a default `reasoning.effort`;
//! 3. reserves the call's worst-case cost against **both** the per-run and
//!    the campaign-wide ledger, refusing with HTTP 402 if it does not fit;
//! 4. settles to the cost the upstream reports (pricing token counts at the
//!    ceiling when it reports none, keeping the whole reservation when it
//!    reports nothing), even if the client disconnected mid-call;
//! 5. appends a row to `<run_dir>/meter.jsonl` tagged with the caller's
//!    phase, seat and candidate.
//!
//! [`Gateway::start`] runs it in-process on its own background thread and
//! Tokio runtime, so a synchronous caller such as the `tinyrsi` CLI can
//! start one per run, hand [`Gateway::tagged_url`] to each client as its
//! OpenAI-compatible base URL, read [`Gateway::spend`] between steps, and
//! [`Gateway::shutdown`] at the end.
//!
//! # Example
//!
//! ```
//! use tinyrsi_gateway::{Gateway, GatewayConfig};
//!
//! let dir = tempfile::tempdir()?;
//! let mut config = GatewayConfig::new(
//!     "sk-or-...", // supplied by the caller; the gateway never reads env
//!     dir.path().join("runs/run-01"),
//!     dir.path().join("runs/ledger-total.json"),
//!     5.0,
//!     50.0,
//! );
//! config.upstream = "http://127.0.0.1:9/v1".into(); // nothing is called here
//! let gateway = Gateway::start(config)?;
//! let base = gateway.tagged_url("score", "judge");
//! assert!(base.starts_with("http://127.0.0.1:"));
//! assert!(base.ends_with("/__tag/score/judge/v1"));
//! assert_eq!(gateway.spend().remaining_usd(), 5.0);
//! gateway.shutdown()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # HTTP surface
//!
//! - `POST /v1/chat/completions` and `POST /v1/embeddings`, tagged by the
//!   `x-tinyrsi-phase`, `x-tinyrsi-seat` and `x-tinyrsi-candidate` headers;
//! - the same under `/__tag/{phase}/{seat}/v1/...` for clients that cannot
//!   set headers;
//! - `GET /v1/models` (also under the tag prefix): the allowlist;
//! - `GET /status`: both ledgers as JSON.
//!
//! # What this crate does not do
//!
//! It does not decide *what* to spend on — that is the loop driver's job —
//! and it never exposes the provider key: clients may send any bearer, the
//! key is not written to the ledgers, meter, captures, logs or errors, and
//! the crate never reads it from the environment. It refuses image content
//! because the run configuration is text-only, and it listens on loopback
//! only.

mod error;
mod gateway;
mod ledger;
mod lock;
mod meter;
mod price;
mod proxy;
mod request;
mod usage;

pub use error::{Error, Result};
pub use gateway::{DEFAULT_LISTEN, DEFAULT_UPSTREAM, DRAIN_LIMIT, Gateway, GatewayConfig, Spend};
pub use price::{ANY_PROVIDER, EMBEDDING, GLM_FLASH, Price, PriceTable};
pub use request::Limits;
