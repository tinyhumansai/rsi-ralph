# tinyrsi-swarm

Runs a tinyhivemind hive package as a team of OpenHuman agents that edit
candidate checkouts: the swarm step of one improvement epoch. The `tinyrsi`
CLI calls it once per epoch; it is the library form of the standalone
`ohrsi-swarm` binary from oh-bench-rsi.

```rust,ignore
let package = tinyrsi_swarm::load_package(Path::new("hives/memory-rsi"))?;
for seat in tinyrsi_swarm::editing_seats(&package)? { /* create its checkout */ }
let outcome = tinyrsi_swarm::run_swarm(request, &mut |event| println!("{event:?}"))?;
```

## What one run does

1. **Checks the request** on the caller's thread: the package parses,
   validates and lowers (`tinyhivemind_lang`); the lead is a seat; every
   editing seat (`config.action_dir` = `"{candidate}"`) has a candidate
   checkout and no other seat does; checkouts are existing directories; tool
   keys name a supported capability and known seats.
2. **Starts a dedicated thread** with a 16 MB stack and its own Tokio runtime.
   Swarms in one process run one at a time, because OpenHuman embeds one
   runtime per process.
3. **Builds one OpenHuman agent per seat** on one runtime whose workspace is
   `<run_dir>/runtime` (sessions and transcripts):
   - provider: OpenAI-compatible at `model_base_url(seat)`, so a spend gateway
     can cap and attribute each seat; model from the seat, else
     `default_model`; temperature from `config.temperature`, else
     `temperature`; `config.max_iterations` when set;
   - system prompt: seat prompt, role charter, working directory, every
     shared `context/` document, then [`HIVE_RULES`](src/prompt/mod.rs);
   - tools: the seat's `tools.allow` plus the nine `hivemind_*` tools and any
     tool-key tools; `tools.deny` plus `wait` and `wait_loop`, always; the
     `coding` pack's schemas are advertised;
   - access: editing seats get `Access::full()` in their candidate; the rest
     `Access::readonly()` in the incumbent.
4. **Coordinates** on `tinyhivemind-hives` with SQLite storage
   (`<run_dir>/hive.db`) and the package's own coordinator settings, creating
   every desk with its members.
5. **Sends the brief** to the lead as a direct message, with each
   candidate's checkout path appended, and drains the coordinator until it is
   idle or `wall` elapses. Each seat turn is bounded by `turn`.
6. **Reports** every event to the progress callback and to
   `<run_dir>/events.jsonl`, and returns a `SwarmOutcome`: why it stopped,
   the coordinator's completed/failed/parked counts, and per-seat turns,
   tokens and cost.

Git is the caller's job: create checkouts before, commit or diff them after.

## Events

`events.jsonl` lines are `{"seq", "elapsed_ms", "event": {"kind", ...}}`, the
envelope `apps/rsi-viz` reads. Kinds: `swarm_started`, `agent_message` (the
host-scope transcript, so seat-to-seat direct messages are included),
`tool_call`, `agent_turn`, `swarm_finished`. Fields are documented in
[`src/event/mod.rs`](src/event/mod.rs) and pinned by its tests.

## Secrets

Tool-key secrets and the model key are scrubbed from every event before it
is written or forwarded, hidden from `Debug`, and never written to the run
directory: a tool key is applied only to its seats' in-memory agent config.
Text a model chooses to write into its own transcript is outside this
guarantee.

## Shell and OpenHuman modules

The swarm always builds OpenHuman with its module loader (`openhuman-embed`
feature `modules`). OpenHuman authorizes every `shell` call from TinyBox's
command analysis, which the loader serves; the seat's access then decides
the call (editing seats run under `Access::full`, read-only seats only
Read-class commands). The first shell call in a process loads the TinyBox
release OpenHuman pins (from the user cache, downloading it once if absent).

## Tool keys

The one capability is `web_search` (Cargo feature `web-search`): providers `brave`, `exa`, `gemini`,
`parallel`, `querit`, `tavily`, `tinyfish`. A key is pinned in memory on each of its
seats' agent config (`SearchConfig::pin_host_credential`, OpenHuman #7460),
which puts its provider on the direct route, and `web_search_tool`,
`web_answer_tool` (Gemini grounding) and `web_contents_tool` join their allow
lists. The key is never written to the saved `api_key` fields, never
serialized and prints redacted; OpenHuman's search proxy uses a pinned
config as given and reconfigures its one module per caller, so seats with
different keys never share one.

## Pin requirement

Read-only seats message through the `hivemind_*` tools, so those tools must
be read-only to OpenHuman's security gate. tinyhivemind `c130000` declares
them as acting (`PermissionLevel::Write`), and the pinned OpenHuman refuses
every acting tool in the read-only tier, so a read-only lead cannot send.
tinyhivemind `e68a560` (which also pins this repository's OpenHuman,
`467ab70`) marks them read-only; the integration tests in `tests/swarm.rs`
need it.

## Lessons carried over from the first live run

| Problem | What this crate does |
| --- | --- |
| Coordinator default `round_width` 1 let a re-nudged episode starve the implementer | uses the package's `coordinator` settings |
| Seats waited or polled inside their turn and held their slot forever | denies `wait`/`wait_loop`; every prompt ends with the async-turn rule |
| A desk episode re-nudged the lead every round | the brief is a direct message to the lead |
| The trace missed seat-to-seat messages | mirrors the host-scope transcript |
| The watcher and the final flush could report a message twice or skip one | both share one cursor under one lock |

## Layout

| Module | Holds |
| --- | --- |
| `package/` | `load_package`, `editing_seats`, the lowered hive with roles and charters |
| `prompt/` | seat system prompts and `HIVE_RULES` |
| `swarm/` | `run_swarm`, request checks, seat specs, the async run |
| `hooks/` | turn hooks: tool-call progress and per-seat usage |
| `mirror/` | transcript mirroring |
| `event/` | `SwarmEvent` and secret scrubbing |
| `trace/` | the `events.jsonl` sink |
| `tools/` | tool keys and web-search wiring |
| `error/` | the crate `Error` |

Tests: unit tests beside each module; `tests/swarm.rs` runs whole swarms
offline against a scripted loopback model (`tests/fixtures/pair`).
