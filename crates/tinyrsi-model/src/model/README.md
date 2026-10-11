# Model boundary

Every model call this crate makes goes through `ModelPort`. The module's job
is to make spending predictable: each request reserves a conservative token
budget before it is sent, every call (including failures) is metered, and
credentials never leave the Rust process.

| File | Responsibility |
| --- | --- |
| `mod.rs` | `ModelRequest` validation and reservation, `OpenRouterModel`, `ModelConversant`, `BudgetedModel` |
| `types.rs` | `ModelPort`, `ModelFuture`, `ModelIdentity`, `ModelRequest`, `ModelReply`, `GenerationMeter` and the adapter structs |
| `test.rs` | Reservation arithmetic, cap enforcement, credential handling |

## Public surface

| Item | Role |
| --- | --- |
| `ModelPort` | Trait: `generate(ModelRequest) -> ModelFuture`, optional `identity()` |
| `ModelRequest::new(system, prompt, seed, max_tokens, max_output_tokens, timeout_ms)` | Frozen request; `with_tool(name, schema)` adds a native structured tool, which `tool()` exposes to the port |
| `ModelReply` | `content` bytes plus measured `input_tokens` / `output_tokens` |
| `OpenRouterModel::new(model_id)` | Bundled HTTPS provider |
| `BudgetedModel::new(port, cap)` | Wraps a port with a hard total reservation cap; `meter()` reports spend |
| `ModelConversant::new(port, seed, total, call, output, timeout)` | tinybackrooms `Conversant` used by the retreat Engine; `meter()` reports spend |
| `GenerationMeter` | `cap`, `reserved_tokens`, `attempted_calls`, `failed_calls`, `denied_calls`, `input_tokens`, `output_tokens` |

## Reservation

`ModelRequest` freezes seed, total tokens, output tokens and timeout. It reserves
one token per UTF-8 byte plus 512 framing tokens, including any native tool
schema, before dispatch. This is conservative for the configured OpenRouter
chat models. A provider with another tokenizer must establish its own upper
bound and honor this contract. Measured usage beyond the bound is rejected.

For example, a 1,000-byte system prompt and a 9,000-byte prompt with a
2,048-token output cap reserve 1,000 + 9,000 + 512 + 2,048 = 12,560 tokens
(`ModelRequest::reservation`). `ModelRequest::new` rejects the request if
that exceeds its own `max_tokens`. `BudgetedModel` and `ModelConversant`
deny a call whose reservation would exceed their remaining cap; it is
counted in `denied_calls` and never reaches the provider.

## OpenRouter

`OpenRouterModel` reads the inherited `OPENROUTER_API_KEY` only inside the Rust
process at call time. It sends credentials and private input through curl's
stdin config, removes the key from the child environment, and withholds HTTP
errors and provider payloads. A dedicated worker keeps blocking HTTPS off the
async executor; the provider timeout bounds an already-dispatched call even if
its future is dropped. Call cancellation does not refund its reservation.

## Engine adapter

`ModelConversant` implements tinybackrooms free turns and native structured tool
calls. Its shared meter covers every exploration, formatting, repair and scorer
call. Failed calls retain reservations. It exposes only counts and successful
measured usage. `reserved_tokens` is the conservative spend limit when a failed
provider call has unknown usage. Models are frozen in the underlying port;
per-request model overrides fail closed.

The engine retains conversational roles in the serialized user history. Sampling
is deterministic at the provider boundary (temperature zero); comparison arms
use the same configuration. A backend with stochastic temperature support can
implement `ModelPort` while keeping the same reservation contract.

## Operational constraints

- `OpenRouterModel` shells out to `curl`; it must be on `PATH`.
- Never log a `ModelRequest` body: it can contain private suite prompts
  during evaluation. Its `Debug` impl omits the text.
- Reservations are conservative by design, so `reserved_tokens` is always
  at least the measured `input_tokens + output_tokens`. Budget experiments
  against `reserved_tokens`, which is what the caps enforce.
- Live calls are only made by opt-in examples; all tests use scripted ports.
