//! Bounded, metered model calls.
//!
//! Every model call in `tinyrsi` goes through [`ModelPort`]: a request
//! ([`ModelRequest`]) reserves a conservative token budget before it is sent,
//! every call is metered ([`GenerationMeter`]), and credentials never leave the
//! process. [`OpenRouterModel`] is the bundled provider, [`BudgetedModel`] caps
//! total spend, and [`ModelConversant`] adapts a port to the tinybackrooms
//! `Conversant` used by the retreat engine. [`block_on`] lets a synchronous
//! loop drive a port's future without an async runtime.
//!
//! # Example
//!
//! ```
//! use rsi_ralph_model::{Error, ModelRequest};
//!
//! let request = ModelRequest::new("system".into(), "prompt".into(), 7, 2048, 100, 1000)?;
//! assert_eq!(request.seed(), 7);
//! // Prompt bytes, 512 framing tokens and the output cap must fit the total.
//! assert_eq!(
//!     ModelRequest::new("system".into(), "prompt".into(), 7, 100, 100, 1000).unwrap_err(),
//!     Error::InvalidRequest
//! );
//! # Ok::<(), rsi_ralph_model::Error>(())
//! ```
//!
//! # What this crate does not hold
//!
//! No prompts, no scoring and no proposal logic. Callers build the request;
//! this crate only bounds, sends and meters it.

mod error;
mod executor;
mod model;

pub use error::{Error, Result};
pub use executor::block_on;
pub use model::*;
