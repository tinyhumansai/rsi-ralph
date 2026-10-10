# TinyRSI

A host-driven orchestrator for improving tinyhivemind hives through typed
patches, private shadow evaluations and explicit human approval. It also
provides a TinyBus lineage module for existing hosts. It is a two-crate cargo workspace:
`crates/tinyrsi-bus` is the wire contract — member names, payload types, and
the contract version, with no transport and no behavior — and `crates/tinyrsi`
is the implementation, built as both an `rlib` and the `cdylib` TinyBus loads.
A host that only makes calls depends on the contract crate alone and compiles
neither the module nor `tinybus` itself.

## What the module does

The Rust API runs a host-controlled improvement loop: persist redacted epoch
observations, generate typed proposals, freeze normalized evaluation policy and
private suite identities, evaluate in isolated coordinator episodes, and ask a
human to approve the exact confirmed candidate. `Archive` retains original
language `Record` and `Package` values, prepared snapshots, declarations, paired
runs, recomputed comparisons, explicit decisions and final outcomes. Selection
and live deployment remain separate durable facts.

The optional `host` feature supplies `CoordinatorHost` over real tinyhivemind
storage, including SQLite, and `DeploymentStore` for activation, compensation,
restart and parent rollback. `SnapshotMemoryStore` provides immutable local
memory images. `VerifiedRunner` settles work only through immutable host checks;
`CoordinatorEvaluator` runs frozen private suites with fresh coordinator storage
per task, seed and arm. `EpochJournal` preserves real retreat attempts and
prevents completed generation/evaluation from repeating after restart.

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

The existing bus API remains compatible:

- **`Append`** records one graded candidate: its position in the lineage, the
  content address of its environment snapshot, its private grade, the verdict
  (`Accepted` or `Rejected`), and the rationale for the decision. An entry
  whose step does not exactly continue the ledger, or whose snapshot address
  or rationale is empty, is rejected.
- **`Head`** answers which entry is the current incumbent: the last accepted
  entry, or `None` before the first acceptance.

The interface is `ai.tinyhumans.tinyrsi.Lineage` at
`/ai/tinyhumans/tinyrsi/Lineage`; every payload type and member name is
published by `crates/tinyrsi-bus`, so a host names them from a library rather
than by string literal.

## Project Structure

```text
Cargo.toml              # virtual workspace: members, [workspace.package],
                        # [workspace.dependencies], [workspace.lints]
crates/
├── tinyrsi-bus/        # the wire contract: what crosses the bus, nothing else
│   └── src/
│       ├── lib.rs      # crate docs + the entire public re-export surface
│       ├── names/      # interface, object path, one constant per member
│       ├── lineage/    # the payload family the members carry
│       └── version/    # contract version and the host bind rule
└── tinyrsi/            # the module: behavior, adapter, and the cdylib
    ├── src/
    │   ├── lib.rs      # crate docs + public surface, re-exporting the contract
    │   ├── error/      # crate-wide `Error` and `Result<T>`
    │   ├── lineage/    # the compatible bus ledger
    │   ├── archive/    # replay-validated proposals and evaluation evidence
    │   ├── observation/, evaluation/, orchestrator/  # epoch and approval funnel
    │   ├── scoring/, verification/  # normalized scores and real host checks
    │   ├── memory/, retreat_memory/ # immutable snapshots and durable discovery
    │   ├── epoch_driver/, retreat/  # actual generation and durable attempts
    │   ├── host/, model/, suites/   # deployment and isolated model evaluation
    │   ├── experiment/ # preregistered paired trial ledger
    │   └── tinybus_module/   # TinyBus interface, ABI exports, integration tests
    ├── tests/          # integration tests against the public API only
    └── examples/       # runnable, compiled-in-CI usage examples
vendor/tinybus/         # pinned TinyBus host types and module SDK
vendor/tinyhivemind/    # pinned language, coordinator and storage APIs
vendor/tinybackrooms/   # pinned creative-retreat engine
docs/
├── rsi/                # RSI research notes and the working framework spec
├── specs/              # behavior and architecture specifications
├── plans/              # test-first implementation plans
└── adr/                # immutable architecture decision records
```

The split is the point. A payload type describes what a frame carries; the
behavior that answers it is a different obligation. `tinyrsi` depends on
`tinyrsi-bus` and re-exports all of it, so `tinyrsi::GradeEntry` and
`tinyrsi_bus::GradeEntry` are the *same* type rather than structural twins,
and a host is never forced to choose between linking the whole module and
redefining the vocabulary. See
[`crates/tinyrsi-bus/README.md`](crates/tinyrsi-bus/README.md).

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
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
cargo run -p tinyrsi --example basic
cargo build -p tinyrsi --release --lib   # produces the installable cdylib
```

Those four checks are exactly what CI runs. Optional extras:

```sh
cargo doc --no-deps --all-features   # CI builds this with RUSTDOCFLAGS="-D warnings"
cargo deny check all                 # supply-chain check; see deny.toml
cargo install cargo-llvm-cov         # once, before running the coverage gate
.github/scripts/check-file-coverage.sh 90 coverage.json
```


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
  cargo run -p tinyrsi --features host --example complete_openrouter
```

It requires inherited `OPENROUTER_API_KEY` and an actual stock Lean executable.
`OPENROUTER_MODEL` chooses the model. `TINYRSI_COMPLETE_DIR` chooses a fresh
host-private directory (default `target/rsi-complete-live`); existing directories
are refused to preserve preregistration. The example spends real API tokens.
It writes protocol and aggregate experiment reports without publishing private
suite contents. A negative or inconclusive report is a valid result.

## Releasing

Run the **Release** workflow from the Actions tab with a `patch`, `minor`, or
`major` bump. Use `current` only to resume an interrupted release whose version
commit and tag already exist. The workflow revalidates the workspace, versions
and tags it — one `[workspace.package]` version that every member inherits —
builds `crates/tinyrsi` as a TinyBus `cdylib`, and creates a GitHub release.
Assets follow `tinyrsi-<version>-<platform>.<tar.gz|zip>` and contain the
native module, its SHA-256 `modules.toml`, license, and
[`MODULE.md`](MODULE.md). Every release also publishes `checksum.toml`, which
TinyBus uses to verify an archive before extraction. The workflow loads the
published Ubuntu archive through TinyBus's GitHub release API and calls its
`Append` method before declaring the release successful. TinyBus itself is not
shipped by this repository; the pinned submodule is the build-time SDK. The
stable native matrix covers Ubuntu 22.04 and 24.04 on x86_64 and ARM64; Fedora
43 and 44 on x86_64 and ARM64; rolling Arch Linux on its officially supported
x86_64 architecture; macOS 15 and 26 on Intel and Apple Silicon; Windows Server
2022 and 2025 on x86_64; and Windows 11 on ARM64.

The separate acceptance/deployment validation uses a model-authored, schema-bound
context repair outside the preregistered comparison:

```sh
TINYRSI_LIVE_OPENROUTER=1 TINYRSI_CONTROLLED_ONLY=1 \
  TINYRSI_COMPLETE_DIR=target/rsi-controlled-live \
  TINYRSI_LEAN=/absolute/path/to/lean \
  cargo run -p tinyrsi --all-features --example complete_openrouter
```

It checks all three normalized evaluation stages, both sandbox gate decisions,
a verified turn on the accepted SQLite deployment, exact parent rollback and
reopening the deployment, archive and runtime. It does not contribute observations
to the frozen original/evolved comparison.
