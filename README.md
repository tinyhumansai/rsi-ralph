# TinyRSI

A TinyBus module that holds the lineage of a recursive self-improvement (RSI)
run driven by OpenHuman agents. It is a two-crate cargo workspace:
`crates/tinyrsi-bus` is the wire contract — member names, payload types, and
the contract version, with no transport and no behavior — and `crates/tinyrsi`
is the implementation, built as both an `rlib` and the `cdylib` TinyBus loads.
A host that only makes calls depends on the contract crate alone and compiles
neither the module nor `tinybus` itself.

## What the module does

The RSI loop (see [`docs/rsi/`](docs/rsi/) for the research and the working
spec) proposes rewrites of an agent's harness configuration, grades each
candidate on a selection benchmark, and keeps only the changes that improve a
private held-out grade. This module is that loop's ledger:

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
    │   ├── lineage/    # the ledger behavior
    │   └── tinybus_module/   # TinyBus interface, ABI exports, integration tests
    ├── tests/          # integration tests against the public API only
    └── examples/       # runnable, compiled-in-CI usage examples
vendor/tinybus/         # pinned TinyBus host types and module SDK
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
