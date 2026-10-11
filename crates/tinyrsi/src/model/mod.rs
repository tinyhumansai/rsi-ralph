//! Bounded model calls and a metered tinybackrooms backend.
#[cfg(test)]
mod test;
mod types;
use crate::{Error, Result};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};
pub use types::*;

impl ModelRequest {
    /// Reserves a conservative byte-based input bound before generation.
    /// # Errors
    /// Rejects empty prompts, invalid limits or a budget below input plus output.
    pub fn new(
        system: String,
        prompt: String,
        seed: u64,
        max_tokens: u64,
        max_output_tokens: u64,
        timeout_ms: u64,
    ) -> Result<Self> {
        let mut request = Self {
            system,
            prompt,
            seed,
            max_tokens,
            max_output_tokens,
            timeout_ms,
            tool: None,
            input_bound: 0,
        };
        request.validate()?;
        Ok(request)
    }
    /// Adds a provider-native structured tool and reserves its schema tokens.
    /// # Errors
    /// Rejects invalid names, schemas or an insufficient execution cap.
    pub fn with_tool(mut self, name: String, schema: Value) -> Result<Self> {
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || !schema.is_object()
        {
            return Err(Error::InvalidEvaluation);
        }
        self.tool = Some((name, schema));
        self.validate()?;
        Ok(self)
    }
    fn validate(&mut self) -> Result<()> {
        let tool_bytes = self
            .tool
            .as_ref()
            .map_or(0, |(n, s)| n.len() + s.to_string().len());
        self.input_bound = u64::try_from(
            self.system
                .len()
                .saturating_add(self.prompt.len())
                .saturating_add(tool_bytes)
                .saturating_add(512),
        )
        .map_err(|_| Error::InvalidEvaluation)?;
        if self.system.is_empty()
            || self.prompt.is_empty()
            || self.max_output_tokens == 0
            || self.timeout_ms == 0
            || self.timeout_ms > 300_000
            || self
                .input_bound
                .checked_add(self.max_output_tokens)
                .is_none_or(|n| n > self.max_tokens)
        {
            return Err(Error::InvalidEvaluation);
        }
        Ok(())
    }
    /// Frozen system instructions, accessible only to the trusted model port.
    #[must_use]
    pub fn system(&self) -> &str {
        &self.system
    }
    /// Task input, accessible only to the trusted model port.
    #[must_use]
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    /// Reproducible request seed.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }
    /// Entire execution token cap, including prompt tokens.
    #[must_use]
    pub fn max_tokens(&self) -> u64 {
        self.max_tokens
    }
    /// Provider-enforced output cap.
    #[must_use]
    pub fn max_output_tokens(&self) -> u64 {
        self.max_output_tokens
    }
    /// Provider timeout in milliseconds.
    #[must_use]
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
    /// Reservation consumed even when the provider fails.
    #[must_use]
    pub fn reservation(&self) -> u64 {
        self.input_bound + self.max_output_tokens
    }
    /// Checks authoritative provider usage against the reserved execution cap.
    /// # Errors
    /// Rejects missing content, overflow, or any reported usage beyond the reservation.
    pub fn check(&self, reply: &ModelReply) -> Result<()> {
        if reply.content.is_empty()
            || reply.input_tokens > self.input_bound
            || reply.output_tokens > self.max_output_tokens
            || reply
                .input_tokens
                .checked_add(reply.output_tokens)
                .is_none_or(|n| n > self.max_tokens)
        {
            return Err(Error::InvalidEvaluation);
        }
        Ok(())
    }
}
impl OpenRouterModel {
    /// Constructs an adapter without reading or retaining credentials.
    /// # Errors
    /// Rejects an empty or control-containing provider model identifier.
    pub fn new(model: String) -> Result<Self> {
        if model.trim().is_empty() || model.chars().any(char::is_control) {
            return Err(Error::InvalidEvaluation);
        }
        Ok(Self { model })
    }
    fn call(&self, request: &ModelRequest) -> Result<ModelReply> {
        let key = std::env::var("OPENROUTER_API_KEY").map_err(|_| Error::HostFailed)?;
        self.call_with_key(
            request,
            &key,
            "https://openrouter.ai/api/v1/chat/completions",
        )
    }
    fn call_with_key(&self, request: &ModelRequest, key: &str, url: &str) -> Result<ModelReply> {
        if key.is_empty() || key.contains(['\r', '\n', '"', '\\']) {
            return Err(Error::HostFailed);
        }
        let body = self.body(request);
        // Credentials AND private prompts go through stdin, never process argv.
        let duration = format!(
            "{}.{:03}",
            request.timeout_ms / 1000,
            request.timeout_ms % 1000
        );
        let config = format!(
            "url = \"{url}\"\nrequest = \"POST\"\nheader = \"Authorization: Bearer {key}\"\nheader = \"Content-Type: application/json\"\ndata = {}\nsilent\nfail\nconnect-timeout = 10\nmax-time = {}\nmax-filesize = 2097152\n",
            serde_json::to_string(&body.to_string()).map_err(|_| Error::HostFailed)?,
            duration
        );
        let mut child = Command::new("curl")
            .args(["--config", "-"])
            .env_remove("OPENROUTER_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Error::HostFailed)?;
        let input_result = child
            .stdin
            .take()
            .ok_or(Error::HostFailed)?
            .write_all(config.as_bytes());
        if input_result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::HostFailed);
        }
        let output = child.wait_with_output().map_err(|_| Error::HostFailed)?;
        if !output.status.success() || output.stdout.len() > 2_097_152 {
            return Err(Error::HostFailed);
        }
        let value: Value = serde_json::from_slice(&output.stdout).map_err(|_| Error::HostFailed)?;
        Self::decode(request, &value)
    }
    fn body(&self, request: &ModelRequest) -> Value {
        let mut body = json!({"model":self.model,"temperature":0,"seed":request.seed,"max_tokens":request.max_output_tokens,"messages":[{"role":"system","content":request.system},{"role":"user","content":request.prompt}]});
        if let Some((name, schema)) = &request.tool {
            body["tools"] =
                json!([{"type":"function","function":{"name":name,"parameters":schema}}]);
            body["tool_choice"] = json!({"type":"function","function":{"name":name}});
        }
        body
    }
    fn decode(request: &ModelRequest, value: &Value) -> Result<ModelReply> {
        let message = &value["choices"][0]["message"];
        let text = if let Some((name, _)) = &request.tool {
            let calls = message["tool_calls"].as_array().ok_or(Error::HostFailed)?;
            if calls.len() != 1 || calls[0]["function"]["name"].as_str() != Some(name.as_str()) {
                return Err(Error::HostFailed);
            }
            calls[0]["function"]["arguments"].as_str()
        } else {
            message["content"].as_str()
        }
        .ok_or(Error::HostFailed)?;
        let reply = ModelReply {
            content: text.as_bytes().to_vec(),
            input_tokens: value["usage"]["prompt_tokens"]
                .as_u64()
                .ok_or(Error::HostFailed)?,
            output_tokens: value["usage"]["completion_tokens"]
                .as_u64()
                .ok_or(Error::HostFailed)?,
        };
        request.check(&reply)?;
        Ok(reply)
    }
}
impl ModelPort for OpenRouterModel {
    fn identity(&self) -> Option<ModelIdentity> {
        Some(ModelIdentity {
            provider: "openrouter".into(),
            model: self.model.clone(),
        })
    }
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        let model = self.clone();
        ModelWorker::start(move || model.call(&request))
    }
}
struct WorkerState {
    result: Option<Result<ModelReply>>,
    waker: Option<std::task::Waker>,
}
struct ModelWorker(Arc<Mutex<WorkerState>>);
impl ModelWorker {
    fn start(operation: impl FnOnce() -> Result<ModelReply> + Send + 'static) -> ModelFuture {
        let shared = Arc::new(Mutex::new(WorkerState {
            result: None,
            waker: None,
        }));
        let worker = shared.clone();
        std::thread::spawn(move || {
            let result = operation();
            if let Ok(mut state) = worker.lock() {
                state.result = Some(result);
                if let Some(waker) = state.waker.take() {
                    waker.wake();
                }
            }
        });
        Box::pin(ModelWorker(shared))
    }
}
impl std::future::Future for ModelWorker {
    type Output = Result<ModelReply>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let Ok(mut state) = self.0.lock() else {
            return std::task::Poll::Ready(Err(Error::HostFailed));
        };
        if let Some(result) = state.result.take() {
            std::task::Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            std::task::Poll::Pending
        }
    }
}
impl ModelConversant {
    /// Constructs a backend with one shared cap covering every engine call.
    /// # Errors
    /// Rejects zero limits or a per-call cap exceeding the total reservation.
    pub fn new(
        port: Arc<dyn ModelPort>,
        seed: u64,
        total_tokens: u64,
        call_tokens: u64,
        output_tokens: u64,
        timeout_ms: u64,
    ) -> Result<Self> {
        if total_tokens == 0
            || call_tokens == 0
            || call_tokens > total_tokens
            || output_tokens == 0
            || timeout_ms == 0
        {
            return Err(Error::InvalidEvaluation);
        }
        Ok(Self {
            port,
            seed,
            call_tokens,
            output_tokens,
            timeout_ms,
            meter: Arc::new(Mutex::new(GenerationMeter {
                cap: total_tokens,
                ..GenerationMeter::default()
            })),
        })
    }
    /// Aggregate accounting without prompts, credentials or model output.
    /// # Errors
    /// Returns a host error if the accounting lock is poisoned.
    pub fn meter(&self) -> Result<GenerationMeter> {
        self.meter
            .lock()
            .map(|m| m.clone())
            .map_err(|_| Error::HostFailed)
    }
    fn request(
        &self,
        system: String,
        messages: &[tinybackrooms::Message],
        tool: Option<(String, Value)>,
    ) -> Result<ModelRequest> {
        let prompt = serde_json::to_string(&messages).map_err(|_| Error::InvalidEvaluation)?;
        let mut req = ModelRequest::new(
            system,
            prompt,
            self.seed,
            self.call_tokens,
            self.output_tokens,
            self.timeout_ms,
        )?;
        if let Some((name, schema)) = tool {
            req = req.with_tool(name, schema)?;
        }
        let mut meter = self.meter.lock().map_err(|_| Error::HostFailed)?;
        let reserved = meter
            .reserved_tokens
            .checked_add(req.reservation())
            .ok_or(Error::InvalidEvaluation)?;
        if reserved > meter.cap {
            meter.denied_calls += 1;
            return Err(Error::InvalidEvaluation);
        }
        meter.reserved_tokens = reserved;
        meter.attempted_calls += 1;
        Ok(req)
    }
    async fn execute(&self, req: ModelRequest) -> Result<ModelReply> {
        let result = self.port.generate(req.clone()).await.and_then(|reply| {
            req.check(&reply)?;
            Ok(reply)
        });
        let mut meter = self.meter.lock().map_err(|_| Error::HostFailed)?;
        match &result {
            Ok(r) => {
                meter.input_tokens += r.input_tokens;
                meter.output_tokens += r.output_tokens;
            }
            Err(_) => meter.failed_calls += 1,
        }
        result
    }
}
fn backend_error(_: Error) -> tinybackrooms::Error {
    tinybackrooms::Error::Conversant("bounded model call failed".into())
}
fn usage(r: &ModelReply) -> tinybackrooms::Usage {
    tinybackrooms::Usage {
        input_tokens: r.input_tokens,
        output_tokens: r.output_tokens,
        calls: 1,
    }
}
impl tinybackrooms::Conversant for ModelConversant {
    fn turn(
        &self,
        r: tinybackrooms::TurnRequest,
    ) -> tinybackrooms::BoxFuture<'_, tinybackrooms::Result<tinybackrooms::TurnReply>> {
        Box::pin(async move {
            if r.model.is_some() {
                return Err(backend_error(Error::InvalidEvaluation));
            }
            let req = self
                .request(r.system, &r.messages, None)
                .map_err(backend_error)?;
            let reply = self.execute(req).await.map_err(backend_error)?;
            Ok(tinybackrooms::TurnReply {
                text: String::from_utf8(reply.content.clone())
                    .map_err(|_| backend_error(Error::HostFailed))?,
                usage: usage(&reply),
            })
        })
    }
    fn structured(
        &self,
        r: tinybackrooms::StructuredRequest,
    ) -> tinybackrooms::BoxFuture<'_, tinybackrooms::Result<tinybackrooms::StructuredReply>> {
        Box::pin(async move {
            if r.model.is_some() {
                return Err(backend_error(Error::InvalidEvaluation));
            }
            let req = self
                .request(r.system, &r.messages, Some((r.tool, r.schema)))
                .map_err(backend_error)?;
            let reply = self.execute(req).await.map_err(backend_error)?;
            Ok(tinybackrooms::StructuredReply {
                value: serde_json::from_slice(&reply.content)
                    .map_err(|_| backend_error(Error::HostFailed))?,
                usage: usage(&reply),
            })
        })
    }
}
impl BudgetedModel {
    /// Wrap a provider with a whole-experiment execution reservation.
    /// # Errors
    /// Rejects a zero token cap.
    pub fn new(port: Arc<dyn ModelPort>, cap: u64) -> Result<Self> {
        Ok(Self {
            backend: ModelConversant::new(port, 0, cap, cap, 1, 1)?,
        })
    }
    /// Public aggregate spend without private prompts or output.
    /// # Errors
    /// Returns a host error if the accounting lock is poisoned.
    pub fn meter(&self) -> Result<GenerationMeter> {
        self.backend.meter()
    }
}
impl ModelPort for BudgetedModel {
    fn identity(&self) -> Option<ModelIdentity> {
        self.backend.port.identity()
    }
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        let backend = self.backend.clone();
        Box::pin(async move {
            {
                let mut meter = backend.meter.lock().map_err(|_| Error::HostFailed)?;
                let total = meter
                    .reserved_tokens
                    .checked_add(request.reservation())
                    .ok_or(Error::InvalidEvaluation)?;
                if total > meter.cap {
                    meter.denied_calls += 1;
                    return Err(Error::InvalidEvaluation);
                }
                meter.reserved_tokens = total;
                meter.attempted_calls += 1;
            }
            backend.execute(request).await
        })
    }
}
