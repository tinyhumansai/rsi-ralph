# TinyRSI

A host-driven orchestrator for improving tinyhivemind hives through typed
patches, private shadow evaluations and explicit human approval, plus a
generic improvement loop for plain scripts. It is a Rust library,
`crates/tinyrsi`, that a host program links; it has no daemon and no plugin
surface of its own.

## What it does

The Rust API runs a host-controlled improvement loop: persist redacted epoch
observations, generate typed proposals, freeze normalized evaluation policy and
private suite identities, evaluate in isolated coordinator episodes, and ask a
human to approve the exact confirmed candidate. `Archive` retains original
language `Record` and `Package` values, prepared snapshots, declarations, paired
runs, recomputed comparisons, explicit decisions and final outcomes. Selection
and live deployment remain separate durable facts.

The `tinyrsi-host` crate supplies `CoordinatorHost` over real tinyhivemind
storage, including SQLite, and `DeploymentStore` for activation, compensation,
restart and parent rollback. `SnapshotMemoryStore` provides immutable local
memory images. `VerifiedRunner` settles work only through immutable host checks;
`CoordinatorEvaluator` runs frozen private suites with fresh coordinator storage
per task, seed and arm. `EpochJournal` preserves real retreat attempts and
prevents completed generation/evaluation from repeating after restart.

`EvidenceJournal` persists controlled turn intent, provider-measured usage and
observed checker evidence, then atomically consumes them into a redacted epoch
receipt. Duplicate callbacks and repeated epoch reads do not recount work after
restart; interrupted calls with missing usage remain explicitly unknown. Run
`cargo run -p tinyrsi-host --example observe_epochs` for the offline
two-epoch demonstration and public receipts under `target/rsi-observe-epochs/`.

`EpochJournal::proposal_dataset` links typed formatter occurrences and committed
predictions to selection-only labels, preserving missing and censored outcomes.
Optional probabilities are frozen before evaluation; Brier, tied-rank Spearman
and precision-at-k report their denominators without using confirmation or anchor
measurements. See the [dataset contract](crates/tinyrsi-host/src/epoch_driver/dataset/README.md)
and the `proposal_metrics` example for reporting an existing private host run.

The supplied `ControlledRunnerFactory` is a bounded, artifact-only, single-seat
adapter. It installs supported prompts, context, recall and conduct caps, and
rejects unsupported models, tools, MCP and policies explicitly. A custom
`RunnerFactory` owns broader deployment capabilities. Multi-seat verified DAG
execution is exercised separately. These adapters use existing upstream APIs;
this repository does not modify OpenHuman or provide a CortexDB memory adapter.

`ExperimentProtocol` and `run_experiment` preregister original/evolved arms,
private draws, equal total caps, ablations and an aggregate reporting rule.
The [complete live run](docs/rsi/complete-live.md) passed separate acceptance and
SQLite deployment/rollback validation; its 24-attempt comparison was inconclusive.
These public toy fixtures do not establish positive RSI or ignition.
See [phase status](docs/rsi/phases.md), [host runtime](docs/specs/host-runtime.md)
and [the experiment protocol](docs/specs/ignition.md).

## Project Structure

```text
Cargo.toml              # virtual workspace: members, [workspace.package],
                        # [workspace.dependencies], [workspace.lints]
crates/
├── tinyrsi-types/      # proposal and candidate data shared by every crate
├── tinyrsi-model/      # bounded, metered model calls
├── tinyrsi-trace/      # live loop trace for rsi-viz and `tracing` subscribers
├── tinyrsi-langfuse/   # Langfuse export of loop traces via openhuman-embed
├── tinyrsi-score/      # Scorer port, LLM/backrooms judges, decide, statistics
├── tinyrsi-verify/     # host checkers, blueprints, pinned Lean, process runner
├── tinyrsi-evolve/     # Evolver port, script supervisor, tinybackrooms retreat
├── tinyrsi-host/       # hive runtime: coordinators, suites, durable epochs, deployment
└── tinyrsi/            # the improvement-loop library
    ├── src/
    │   ├── lib.rs      # crate docs + the entire public surface
    │   ├── error/      # crate-wide `Error` and `Result<T>`
    │   ├── lineage/    # small in-memory ledger of graded candidates
    │   ├── archive/    # replay-validated proposals and evaluation evidence
    │   ├── observation/, orchestrator/  # epoch and approval funnel
    │   ├── memory/     # immutable snapshots
    │   └── script_loop/  # plain-script loop
    ├── tests/          # integration tests against the public API only
    └── examples/       # runnable, compiled-in-CI usage examples
vendor/tinyhivemind/    # pinned language, coordinator and storage APIs
vendor/tinybackrooms/   # pinned creative-retreat engine
vendor/openhuman/       # pinned OpenHuman, for its embed Langfuse client
examples/               # end-to-end examples: math_problem, upgrade_file, hive_goal
docs/
├── rsi/                # RSI research notes and the working framework spec
├── specs/              # behavior and architecture specifications
├── plans/              # test-first implementation plans
└── adr/                # immutable architecture decision records
```

Within each crate, feature areas use directory modules: implementation and
exports live in `mod.rs`, substantial types move to `types.rs`, and unit tests
live in `test.rs`. [`AGENTS.md`](AGENTS.md) holds the complete repository
guidance, and `CLAUDE.md` is a symlink to it so every coding agent reads one
source of truth.

## Development

Rust 1.96 or newer is required by the pinned hive and retreat libraries.
Clone with submodules, or initialize them before building:

```sh
git submodule update --init --recursive
```

```sh
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
cargo run -p tinyrsi --example basic
```

Those four checks are exactly what CI runs. Optional extras:

```sh
cargo doc --no-deps --all-features   # CI builds this with RUSTDOCFLAGS="-D warnings"
cargo deny check all                 # supply-chain check; see deny.toml
cargo install cargo-llvm-cov         # once, before running the coverage gate
.github/scripts/check-file-coverage.sh 90 coverage.json
```


## Loop visualizer

[`apps/rsi-viz`](apps/rsi-viz/README.md) is a read-only Next.js app that lists
runs under `target/` and follows a loop's `events.jsonl` live: current stage,
metric by epoch, proposals with their decisions, and the incumbent lineage.
`cd apps/rsi-viz && npm ci && npm run dev`.

To see the same runs in Langfuse, add a `tinyrsi_langfuse::LangfuseTrace` to the
loop's sink (for example beside `JsonlTrace` in a `FanoutTrace`). It sends
OTLP spans through OpenHuman's own Langfuse client and reads the same
`LANGFUSE_*` variables; see `.env.example`.

## Live OpenRouter smoke test

With an existing `OPENROUTER_API_KEY` in your environment:

```sh
TINYRSI_LIVE_OPENROUTER=1 cargo run -p tinyrsi --example live_openrouter
```

Optional `OPENROUTER_MODEL` selects the model (default `openai/gpt-4.1-mini`).
`TINYRSI_SMOKE_ARCHIVE` selects a new archive path under `target/`; the example
refuses to overwrite an existing run. Credentials never enter arguments or
logs. The test runs the tinybackrooms critic/proposer engine through OpenRouter,
requests a typed context patch, and evaluates a deliberately weak
baseline and candidate on paired arithmetic tasks, confirms on fresh draws,
checks denied approval preserves the baseline, and reopens durable evidence.
It spends real API tokens. Passing proves integration, not general improvement.

## Complete host experiment

The optional example combines durable observation, actual tinybackrooms retreat,
isolated code/reference/schema/Lean verification and the preregistered
original/evolved experiment with memory-off and retreat-off ablations. The separate
controlled path below exercises normalized confirmation, approval and deployment:

```sh
TINYRSI_LIVE_OPENROUTER=1 TINYRSI_LEAN=/absolute/path/to/lean \
  cargo run -p tinyrsi-host --example complete_openrouter
```

It requires inherited `OPENROUTER_API_KEY` and an actual stock Lean executable.
`OPENROUTER_MODEL` chooses the model. `TINYRSI_COMPLETE_DIR` chooses a fresh
host-private directory (default `target/rsi-complete-live`); existing directories
are refused to preserve preregistration. The example spends real API tokens.
It writes protocol and aggregate experiment reports without publishing private
suite contents. A negative or inconclusive report is a valid result.

## End-to-end examples

[`examples/`](examples/README.md) runs the whole loop (evolve, score, keep
what is better) on four problems, offline by default:

- `math_problem`: compute π to 12 digits.
- `upgrade_file`: bring a file up to a written coding standard.
- `graph_coloring`: colour random graphs with as few colours as possible.
- `hive_goal`: improve a hive of agents until they solve private tasks.

Run `cargo run -p tinyrsi-examples --example math_problem` (or
`upgrade_file`, `graph_coloring`, `hive_goal`), then watch the runs in `apps/rsi-viz` with
`RSI_RUN_ROOT=../../target/rsi-examples`.

## Improving a plain script

`run_script_loop` repeats observe, ideate, execute, measure, score and improve
over any script that prints `METRIC <name>=<value>` (plus optional
`DIAG <text>` lines the supervisor reads), supervised by a language
model, and streams every stage to `events.jsonl`. Try it offline on a
curve-fitting script with `cargo run -p tinyrsi --example script_loop`; point
it at your own script with the [script loop guide](docs/guides/script-loop.md).
Proposed code runs locally without a sandbox; read the guide's safety note
before live runs.

## Releasing

Run the **Release** workflow from the Actions tab with a `patch`, `minor`, or
`major` bump. Use `current` only to resume an interrupted release whose version
commit and tag already exist. The workflow revalidates the workspace, versions
and tags it — one `[workspace.package]` version that every member inherits —
and creates a GitHub release with generated notes. A release is a tagged
source version; there are no binary assets.

The separate acceptance/deployment validation uses a model-authored, schema-bound
context repair outside the preregistered comparison:

```sh
TINYRSI_LIVE_OPENROUTER=1 TINYRSI_CONTROLLED_ONLY=1 \
  TINYRSI_COMPLETE_DIR=target/rsi-controlled-live \
  TINYRSI_LEAN=/absolute/path/to/lean \
  cargo run -p tinyrsi-host --example complete_openrouter
```

It checks all three normalized evaluation stages, both sandbox gate decisions,
a verified turn on the accepted SQLite deployment, exact parent rollback and
reopening the deployment, archive and runtime. It does not contribute observations
to the frozen original/evolved comparison.
