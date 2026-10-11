//! Model request reservations and public aggregate accounting.
use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
/// A boxed provider operation.
pub type ModelFuture = Pin<Box<dyn Future<Output = Result<ModelReply>> + Send + 'static>>;
/// The trusted provider boundary; implementations must enforce request caps.
pub trait ModelPort: Send + Sync {
    /// Generates one artifact under frozen host limits.
    fn generate(&self, request: ModelRequest) -> ModelFuture;
    /// Public provider/model configuration, absent when the adapter cannot attest it.
    /// This is host provenance, not proof of a remote service's implementation.
    fn identity(&self) -> Option<ModelIdentity> {
        None
    }
}
/// Public host-configured model provenance; never credentials or response text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelIdentity {
    /// Public provider identifier.
    pub provider: String,
    /// Public immutable model configuration identifier.
    pub model: String,
}
/// Private prompt and immutable limits; debugging excludes all prompt content.
#[derive(Clone)]
pub struct ModelRequest {
    pub(super) system: String,
    pub(super) prompt: String,
    pub(super) seed: u64,
    pub(super) max_tokens: u64,
    pub(super) max_output_tokens: u64,
    pub(super) timeout_ms: u64,
    pub(super) tool: Option<(String, serde_json::Value)>,
    pub(super) temperature: f64,
    pub(super) input_bound: u64,
}
impl std::fmt::Debug for ModelRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelRequest")
            .field("seed", &self.seed)
            .field("max_tokens", &self.max_tokens)
            .field("temperature", &self.temperature)
            .finish_non_exhaustive()
    }
}
/// Host-measured output and token usage; not public evaluation evidence.
#[derive(Clone)]
pub struct ModelReply {
    /// Raw generated artifact bytes.
    pub content: Vec<u8>,
    /// Provider-measured prompt tokens.
    pub input_tokens: u64,
    /// Provider-measured completion tokens.
    pub output_tokens: u64,
}
/// An `OpenRouter` HTTPS adapter using inherited credentials only at call time.
#[derive(Clone, Debug)]
pub struct OpenRouterModel {
    pub(super) model: String,
}
/// Whole-generation accounting, including failed and refused calls.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationMeter {
    /// Frozen whole-generation reservation cap.
    pub cap: u64,
    /// Reservations charged before calls; failures never refund reservations.
    pub reserved_tokens: u64,
    /// Calls actually dispatched.
    pub attempted_calls: u64,
    /// Dispatched calls that failed or violated usage bounds.
    pub failed_calls: u64,
    /// Calls refused before provider execution.
    pub denied_calls: u64,
    /// Measured input tokens of successfully metered responses.
    pub input_tokens: u64,
    /// Measured output tokens of successfully metered responses.
    pub output_tokens: u64,
}
/// Native-tool tinybackrooms backend with shared generation reservations.
#[derive(Clone)]
pub struct ModelConversant {
    pub(super) port: Arc<dyn ModelPort>,
    pub(super) seed: u64,
    pub(super) call_tokens: u64,
    pub(super) output_tokens: u64,
    pub(super) timeout_ms: u64,
    pub(super) meter: Arc<Mutex<GenerationMeter>>,
}

impl std::fmt::Debug for ModelReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelReply")
            .field("input_tokens", &self.input_tokens)
            .field("output_tokens", &self.output_tokens)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for ModelConversant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelConversant")
            .field("seed", &self.seed)
            .field("call_tokens", &self.call_tokens)
            .finish_non_exhaustive()
    }
}
/// Shared provider cap across separate evaluation stages and anchor calls.
#[derive(Clone)]
pub struct BudgetedModel {
    pub(super) backend: ModelConversant,
}
impl std::fmt::Debug for BudgetedModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BudgetedModel").finish_non_exhaustive()
    }
}
