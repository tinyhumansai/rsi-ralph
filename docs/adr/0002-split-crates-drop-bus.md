# 2. Split the library by loop phase and drop the TinyBus module

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

The workspace started from a template whose shape was a TinyBus module: a
payload contract crate (`tinyrsi-bus`) and a module crate built as a `cdylib`
and released for 16 platforms. The improvement loop then grew inside the module
crate until it held about 16k lines. The bus surface is a two-method lineage
ledger, and no host consumes it. The loop is used as a Rust library.

Inside that one crate, the code already falls into phases with one-way
dependencies, with one exception: `archive` and `orchestrator` refer to each
other through a handful of shared data types. The phases are:

- statistics (`evaluation`, `scoring`);
- checkers (`process`, `verification`);
- proposal generation (`retreat`, the script supervisor);
- the run loops;
- the host runtime, which is already a Cargo feature.

## Decision

- Remove the TinyBus module, its contract crate, the `vendor/tinybus`
  submodule and the module release packaging.
- Split the library into crates along the loop's phases, as the
  [crate-split specification](../specs/crate-split.md) describes:
  - `tinyrsi-types`, `tinyrsi-model` and `tinyrsi-trace` (shared foundations);
  - `tinyrsi-score` and `tinyrsi-verify` (scoring);
  - `tinyrsi-evolve` (evolution);
  - `tinyrsi` (the loops);
  - `tinyrsi-host` (the hive runtime, replacing the `host` feature).
- Scoring and evolution are synchronous traits (`Scorer`, `Evolver`) that the
  loop calls. A scorer produces qualities. The existing `compare` and
  `compare_h` remain the only decision rules.

## Consequences

- Each phase builds and tests alone. Scoring and evolution cannot acquire a
  dependency on the coordinator or SQLite without a visible manifest change.
- New scorers, including judges, and new evolvers are added without editing the
  loop.
- The bus items (`INTERFACE`, `METHODS`, `OBJECT_PATH`, `CONTRACT_VERSION`,
  `is_compatible`, the `Lineage*` request and response types) and the `host`
  feature are removed. Items that needed the `host` feature move to
  `tinyrsi_host::` paths. This is a breaking change and needs a minor bump.
- Releases no longer ship native module archives. A release is a tagged source
  version.
- There are more manifests to maintain, and a change that spans phases touches
  more than one crate.
