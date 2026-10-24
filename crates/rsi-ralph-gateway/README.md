# rsi-ralph-gateway

The single spend choke point for a `rsi-ralph` run: an OpenAI-compatible
loopback proxy to OpenRouter that refuses any call whose worst-case cost does
not fit in **both** the run's budget and the campaign-wide budget.

The `rsi-ralph` CLI starts one gateway in-process per run and hands each client
(agents, judges, embedders) a tagged base URL instead of the real key.

## Usage

```rust
use rsi_ralph_gateway::{Gateway, GatewayConfig};

let config = GatewayConfig::new(
    api_key,                     // passed in; this crate never reads env
    "runs/run-01",               // budget.json, meter.jsonl, captures/
    "runs/ledger-total.json",    // shared by every run
    5.0,                         // run cap, USD
    50.0,                        // total cap, USD
);
let gateway = Gateway::start(config)?;          // synchronous; returns once listening
let base = gateway.tagged_url("score", "judge"); // http://127.0.0.1:PORT/__tag/score/judge/v1
// ... point an OpenAI-compatible client at `base` with any bearer ...
println!("${:.4} left", gateway.spend().remaining_usd());
gateway.shutdown()?;                              // or just drop it
```

`GatewayConfig::new` defaults `listen` to `127.0.0.1:0` (an OS-chosen port),
`upstream` to `https://openrouter.ai/api/v1`, `prices` to
`PriceTable::standard()`, `limits` to `Limits::default()` and `capture` to
off. Override any field before calling `start`.

## Public surface

| Item | What it is |
|---|---|
| `Gateway::start(config)` | Claims the total ledger, opens both ledgers and the meter, binds, and serves on a background thread with its own Tokio runtime. Works from sync and async callers. |
| `Gateway::base_url()` / `addr()` | `http://127.0.0.1:PORT` and the bound address. |
| `Gateway::tagged_url(phase, seat)` | `<base>/__tag/<phase>/<seat>/v1`, percent-encoded. |
| `Gateway::spend()` | `Spend { run_usd, total_usd, run_limit_usd, total_limit_usd }`, read in-process; `Spend::remaining_usd()` is the tighter of the two caps. |
| `Gateway::upstream_calls()` | `UpstreamCalls { sent, refused, last_refusal }`: calls forwarded to the provider and how many it refused (a non-success status, or no answer as `502`), with the latest `Refusal { status, message }`. Calls the gateway refuses itself are not counted. |
| `Gateway::call_mark()` / `upstream_calls_since(mark)` | A `CallMark` before a phase, then that phase's calls alone. Each refusal counts against the call it answers, so a call sent before the mark and refused after it stays out. |
| `Gateway::shutdown()` | Stops accepting, finishes open connections, waits up to `DRAIN_LIMIT` for calls still settling, releases the lock. `Drop` does the same. |
| `PriceTable`, `Price`, `Limits` | The model allowlist with per-token price ceilings and provider pins; completion-length and reasoning-effort limits. |
| `Error`, `Result` | One `thiserror` enum for every refusal and failure. |

## HTTP surface

- `POST /v1/chat/completions`, `POST /v1/embeddings` — tagged by the
  `x-tinyrsi-phase`, `x-tinyrsi-seat` and `x-tinyrsi-candidate` headers.
- The same under `/__tag/{phase}/{seat}/v1/...`, for clients that can only be
  given a base URL. `x-tinyrsi-candidate` still applies.
- `GET /v1/models` (also under the tag prefix) — the allowlist.
- `GET /status` — both ledgers as JSON.

Refusals are JSON `{"error": {"message", "code"}}`: 402 for a budget that
does not fit or a breached price bound, 403 for a model off the allowlist,
400 for a malformed body or image content, 502 when the upstream cannot be
reached. Upstream statuses and bodies otherwise pass through.

## How a call is charged

1. **Prepare.** The model must be in the `PriceTable`. The body is pinned to
   the model's provider (`provider.only`, `allow_fallbacks: false`,
   `provider.max_price` at the ceiling), asked to report cost
   (`usage.include`, and `stream_options.include_usage` for streams),
   `max_tokens` is defaulted and clamped, and `reasoning_effort` is rewritten
   to `reasoning.effort`, defaulting to `low` when the client names none.
2. **Reserve** the worst case — prompt bytes plus slack at the prompt
   ceiling, plus `max_tokens` at the completion ceiling — against both
   ledgers. Either one short: HTTP 402, nothing is sent upstream.
3. **Forward** with the real key. Streams pass through as they arrive while
   the usage event is scanned out of them. Connection-establishment failures
   are retried up to twice using fresh connections. Request errors after
   connecting, including a disconnect before response headers, are ambiguous
   and are not replayed; settlement retains their reservation as uncertain.
4. **Settle** to the reported cost; to the token counts priced at the ceiling
   if no cost is reported; to zero if the upstream rejected the request with a
   4xx; and to the whole reservation if nothing is known (crash, unreachable
   upstream). A cost above its reservation sets `breached`, which refuses
   every later call. Settlement runs detached, so it completes even when the
   client disconnects mid-call.
5. **Meter** one JSON line in `<run_dir>/meter.jsonl` per call, admitted or
   refused, with its tags, model, status, reservation, cost and tokens.
6. **Tally** every forwarded call in memory: counted as sent when it is
   forwarded, and as refused once its answer is a non-success status or never
   completes (a failed connection, or a body that broke off, counted as
   `502`). The provider's message (its
   `error.message`, else the body) is kept for the latest refusal, on one
   line and at most 300 characters, after key redaction.

Ledgers are JSON files written atomically after every change. A reservation
left in flight by a crashed process becomes `uncertain` (still charged) the
next time the ledger opens; the limit of an existing ledger can never change.
`<total_ledger>.lock` keeps a second gateway off the same total ledger.

With `capture` set, every forwarded body is written to
`<run_dir>/captures/<seq>-<phase>.json`, for seeing what a client actually
sends.

## Constraints

- **The key stays secret.** It is sent upstream as the bearer and nowhere
  else: not in ledgers, meter rows, captures, logs, errors or `Debug` output.
  Upstream text that echoes it is redacted before it is metered or returned.
- **Loopback only.** `start` refuses a non-loopback `listen` address.
- **Text only.** Chat messages with non-text content parts are refused.
- **Conservative accounting.** Unknown and orphaned costs stay charged; that
  is the intended failure direction.
- Capture and meter write failures are reported through `tracing` and do not
  fail a call that was already charged.
