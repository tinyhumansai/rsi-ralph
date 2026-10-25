# Developing RSI Ralph

How to build, test and release this repository. The working agreement for
humans and coding agents (style, errors, dependencies, testing, pull requests)
is [`AGENTS.md`](../AGENTS.md); `CLAUDE.md` is a symlink to it. For the shape
of the system read [architecture.md](architecture.md) first.

## Project structure

```text
Cargo.toml              # virtual workspace: members, [workspace.package],
                        # [workspace.dependencies], [workspace.lints]
crates/
├── rsi-ralph-types/      # proposal and candidate data shared by every crate
├── rsi-ralph-model/      # bounded, metered model calls
├── rsi-ralph-trace/      # live loop trace for rsi-viz and `tracing` subscribers
├── rsi-ralph-langfuse/   # Langfuse export of loop traces via openhuman-embed
├── rsi-ralph-gateway/    # spend-capped loopback proxy to OpenRouter, one per run
├── rsi-ralph-score/      # Scorer port, LLM/backrooms judges, decide, statistics
├── rsi-ralph-verify/     # host checkers, blueprints, pinned Lean, process runner
├── rsi-ralph-evolve/     # Evolver port, script supervisor, tinybackrooms retreat
├── rsi-ralph-cli/        # the `rsi-ralph` binary: file-defined loops, run store, UI
├── rsi-ralph-host/       # hive runtime: coordinators, suites, durable epochs, deployment
├── rsi-ralph-swarm/      # a hive package run as OpenHuman agents editing candidates
└── rsi-ralph/            # the improvement-loop library
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
loops/                  # loop folders; `rsi-ralph init` compiles these in as templates
scripts/                # install.sh, install.ps1 and their tests; benchmark curation
vendor/tinyhivemind/    # pinned language, coordinator and storage APIs
vendor/tinybackrooms/   # pinned creative-retreat engine
vendor/openhuman/       # pinned OpenHuman, for its agents and Langfuse client
examples/               # end-to-end examples: math_problem, upgrade_file, hive_goal
apps/rsi-viz/           # read-only Next.js loop visualizer
docs/                   # see README.md in this folder
```

Within each crate, feature areas use directory modules: implementation and
exports live in `mod.rs`, substantial types move to `types.rs`, and unit tests
live in `test.rs`.

## Building and testing

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
cargo run -p rsi-ralph --example basic
```

Those four checks are exactly what CI runs. Optional extras:

```sh
cargo doc --no-deps --all-features   # CI builds this with RUSTDOCFLAGS="-D warnings"
cargo deny check all                 # supply-chain check; see deny.toml
cargo install cargo-llvm-cov         # once, before running the coverage gate
.github/scripts/check-file-coverage.sh 90 coverage.json
python3 -m unittest discover -s scripts/install_tests -p 'test_*.py'   # install.sh
```

To use a local build of the binary as `rsi-ralph`, run
`cargo install --path crates/rsi-ralph-cli`, or prefix commands with
`cargo run -p rsi-ralph-cli --`.

## Loop visualizer

[`apps/rsi-viz`](../apps/rsi-viz/README.md) is a read-only Next.js app that lists
runs under `target/` and follows a loop's `events.jsonl` live: current stage,
metric by epoch, proposals with their decisions, and the incumbent lineage.
`cd apps/rsi-viz && npm ci && npm run dev`. The `rsi-ralph` binary serves its
own embedded UI and does not need it.

To see the same runs in Langfuse, add a `rsi_ralph_langfuse::LangfuseTrace` to the
loop's sink (for example beside `JsonlTrace` in a `FanoutTrace`). It sends
OTLP spans through OpenHuman's own Langfuse client and reads the same
`LANGFUSE_*` variables; see `.env.example`.

## Live runs

### OpenRouter smoke test

With an existing `OPENROUTER_API_KEY` in your environment:

```sh
TINYRSI_LIVE_OPENROUTER=1 cargo run -p rsi-ralph --example live_openrouter
```

Optional `OPENROUTER_MODEL` selects the model (default `openai/gpt-4.1-mini`).
`TINYRSI_SMOKE_ARCHIVE` selects a new archive path under `target/`; the example
refuses to overwrite an existing run. Credentials never enter arguments or
logs. The test runs the tinybackrooms critic/proposer engine through OpenRouter,
requests a typed context patch, and evaluates a deliberately weak
baseline and candidate on paired arithmetic tasks, confirms on fresh draws,
checks denied approval preserves the baseline, and reopens durable evidence.
It spends real API tokens. Passing proves integration, not general improvement.

### Complete host experiment

The optional example combines durable observation, actual tinybackrooms retreat,
isolated code/reference/schema/Lean verification and the preregistered
original/evolved experiment with memory-off and retreat-off ablations:

```sh
TINYRSI_LIVE_OPENROUTER=1 TINYRSI_LEAN=/absolute/path/to/lean \
  cargo run -p rsi-ralph-host --example complete_openrouter
```

It requires inherited `OPENROUTER_API_KEY` and an actual stock Lean executable.
`OPENROUTER_MODEL` chooses the model. `TINYRSI_COMPLETE_DIR` chooses a fresh
host-private directory (default `target/rsi-complete-live`); existing directories
are refused to preserve preregistration. The example spends real API tokens.
It writes protocol and aggregate experiment reports without publishing private
suite contents. A negative or inconclusive report is a valid result.

### Controlled acceptance and deployment

The separate acceptance/deployment validation uses a model-authored, schema-bound
context repair outside the preregistered comparison:

```sh
TINYRSI_LIVE_OPENROUTER=1 TINYRSI_CONTROLLED_ONLY=1 \
  TINYRSI_COMPLETE_DIR=target/rsi-controlled-live \
  TINYRSI_LEAN=/absolute/path/to/lean \
  cargo run -p rsi-ralph-host --example complete_openrouter
```

It checks all three normalized evaluation stages, both sandbox gate decisions,
a verified turn on the accepted SQLite deployment, exact parent rollback and
reopening the deployment, archive and runtime. It does not contribute observations
to the frozen original/evolved comparison.

## Releasing

Run the **Release** workflow from the Actions tab with a `patch`, `minor`, or
`major` bump. Use `current` only to resume an interrupted release whose version
commit and tag already exist. The workflow revalidates the workspace, then
versions and tags it with the one `[workspace.package]` version that every
member inherits.

It then builds `rsi-ralph` on native runners for five targets and creates the
GitHub release with every archive attached in the same call, because a
published release is immutable and takes no assets afterwards:

| Target | Runner | Archive |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-22.04` | `rsi-ralph-x86_64-unknown-linux-gnu.tar.gz` |
| `aarch64-unknown-linux-gnu` | `ubuntu-22.04-arm` | `rsi-ralph-aarch64-unknown-linux-gnu.tar.gz` |
| `x86_64-apple-darwin` | `macos-latest` | `rsi-ralph-x86_64-apple-darwin.tar.gz` |
| `aarch64-apple-darwin` | `macos-latest` | `rsi-ralph-aarch64-apple-darwin.tar.gz` |
| `x86_64-pc-windows-msvc` | `windows-latest` | `rsi-ralph-x86_64-pc-windows-msvc.zip` |

Each archive holds one `rsi-ralph-<target>/` folder with the binary, `LICENSE`
and `README.md`, and has a `.sha256` file beside it. The names carry no
version, so `releases/latest/download/<archive>` always resolves to the newest
build; [`scripts/install.sh`](../scripts/install.sh) and
[`scripts/install.ps1`](../scripts/install.ps1) depend on exactly these names.
The Linux builds link the system's glibc (2.35 or newer) and OpenSSL 3, which
OpenHuman's HTTP client uses there.

Last, the workflow publishes the container image
`ghcr.io/tinyhumansai/rsi-ralph`, tagged with the version and `latest`. Build
it yourself with `docker build -t rsi-ralph .` from a checkout with submodules
initialized. The crates are not on crates.io: they depend on vendored git and
path dependencies.

CI's `binary-platforms` job builds and smoke-tests the binary on macOS and
Windows on every push, so a release is not the first build on those platforms.
