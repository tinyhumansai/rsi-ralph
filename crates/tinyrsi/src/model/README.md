# Model boundary

`ModelRequest` freezes seed, total tokens, output tokens and timeout. It reserves
one token per UTF-8 byte plus 512 framing tokens, including any native tool
schema, before dispatch. This is conservative for the configured OpenRouter
chat models. A provider with another tokenizer must establish its own upper
bound and honor this contract. Measured usage beyond the bound is rejected.

`OpenRouterModel` reads the inherited `OPENROUTER_API_KEY` only inside the Rust
process at call time. It sends credentials and private input through curl's
stdin config, removes the key from the child environment, and withholds HTTP
errors and provider payloads. A dedicated worker keeps blocking HTTPS off the
async executor; the provider timeout bounds an already-dispatched call even if
its future is dropped. Call cancellation does not refund its reservation.

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
