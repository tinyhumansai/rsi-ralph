//! Evolution: given the incumbent and the goal, propose what to try next.
//!
//! An [`Evolver`] reads what it needs about the incumbent and returns ranked
//! proposals ([`Proposed`]). It never builds or runs candidates; the loop that
//! owns the subject applies a proposal, measures the result and decides.
//!
//! - [`ScriptSupervisor`] proposes whole-file rewrites of a script through one
//!   forced tool call to a [`tinyrsi_model::ModelPort`].
//! - [`RetreatEvolver`] runs the tinybackrooms critic-pair retreat to propose
//!   typed hive patches. [`DurableRetreatMemory`] gives that engine recall
//!   across runs.
//! - [`ScriptedEvolver`] replays canned proposals, for tests and offline runs.
//!
//! # Example
//!
//! ```
//! use tinyrsi_evolve::{EvolveContext, Evolver, ScriptedEvolver};
//! use tinyrsi_types::Goal;
//!
//! let evolver: ScriptedEvolver<str, &str> = ScriptedEvolver::one_each(["try a smaller step"]);
//! let context = EvolveContext { epoch: 1, index: 0, seed: 7 };
//! let proposed = evolver.propose("incumbent", &Goal::new("converge"), &context)?;
//! assert_eq!(proposed.proposals[0], Ok("try a smaller step"));
//! # Ok::<(), tinyrsi_evolve::Error>(())
//! ```
//!
//! # What this crate does not hold
//!
//! No scoring, no execution and no loop: an evolver's proposals are only
//! suggestions until the loop has measured them.

mod error;
mod port;
mod retreat;
mod retreat_memory;
mod script;
mod scripted;

pub use error::{Error, Result};
pub use port::{EvolveContext, Evolver, Proposed};
pub use retreat::{
    RetreatBrief, RetreatDossier, RetreatEvolver, RetreatOutput, RetreatTransport,
    build_retreat_request, decode_retreat_response, generate_retreat, generate_retreat_with_engine,
    typed_retreat_request,
};
pub use retreat_memory::*;
pub use script::{
    ProposedFile, ScriptBrief, ScriptProposal, ScriptSupervisor, SupervisorLimits,
    is_plain_relative,
};
pub use scripted::ScriptedEvolver;
