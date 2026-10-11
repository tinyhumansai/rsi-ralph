//! Runs a tinyhivemind hive package as a team of `OpenHuman` agents that edit
//! candidate checkouts — the "swarm" step of one improvement epoch.
//!
//! A hive package (hive language: `hive.json`, `constitution.json`,
//! `memory.json`, and `roles/`, `seats/`, `context/` Markdown) declares seats.
//! [`run_swarm`] builds one `OpenHuman` agent per seat on a single embedded
//! runtime, registers them on a durable tinyhivemind coordinator, sends the
//! epoch brief to the lead seat as a direct message, and lets the seats
//! message each other until the lead stops (the coordinator goes idle) or
//! the wall clock runs out.
//!
//! - Seats whose `config.action_dir` is `"{candidate}"` ([`editing_seats`])
//!   get full access to their own candidate checkout; the rest read the
//!   incumbent checkout.
//! - Every model call goes to the per-seat base URL the caller supplies, so
//!   a spend gateway can cap and attribute it.
//! - Events stream to a progress callback and to `events.jsonl`; tool
//!   secrets never reach either.
//!
//! # Example
//!
//! ```no_run
//! use std::{collections::BTreeMap, path::Path, sync::Arc, time::Duration};
//! use tinyrsi_swarm::{SwarmRequest, editing_seats, load_package, run_swarm};
//!
//! let package = load_package(Path::new("hives/memory-rsi"))?;
//! let candidates: BTreeMap<_, _> = editing_seats(&package)?
//!     .into_iter()
//!     .map(|seat| {
//!         let checkout = Path::new("runs/epoch-1/candidates").join(&seat);
//!         (seat, checkout)
//!     })
//!     .collect();
//! let outcome = run_swarm(
//!     SwarmRequest {
//!         package,
//!         lead: "lead".into(),
//!         brief: "Improve recall on long histories.".into(),
//!         run_dir: "runs/epoch-1/swarm".into(),
//!         incumbent: "vendor/tinymemory".into(),
//!         candidates,
//!         model_base_url: Arc::new(|seat| format!("http://127.0.0.1:8790/__tag/swarm/{seat}/v1")),
//!         model_api_key: "gateway".into(),
//!         default_model: "z-ai/glm-5.3-flash".into(),
//!         temperature: Some(0.0),
//!         tools: Vec::new(),
//!         wall: Duration::from_secs(3600),
//!         turn: Duration::from_secs(1200),
//!     },
//!     &mut |event| println!("{event:?}"),
//! )?;
//! println!("{:?} after {} turns", outcome.stop, outcome.completed);
//! # Ok::<(), tinyrsi_swarm::Error>(())
//! ```
//!
//! # What this crate does not hold
//!
//! No git: the caller creates candidate checkouts before the run and
//! commits, diffs or discards them after it. No scoring, gating or
//! promotion, and no spend ledger: those belong to the epoch driver and the
//! gateway it runs. It is its own crate because it links the whole
//! `OpenHuman` agent runtime, which hosts that only score or trace should not
//! compile.

mod error;
mod event;
mod hooks;
mod mirror;
mod package;
mod prompt;
mod swarm;
mod tools;

pub use error::{Error, Result};
pub use event::{SeatInfo, StopReason, SwarmEvent, ToolPhase};
pub use package::{CANDIDATE_DIR, editing_seats, load_package};
pub use prompt::HIVE_RULES;
pub use swarm::{ModelUrl, RUNTIME_SEAT, SeatUsage, SwarmOutcome, SwarmRequest, run_swarm};
pub use tools::{SEARCH_PROVIDERS, SEARCH_TOOLS, ToolKey, WEB_SEARCH};
