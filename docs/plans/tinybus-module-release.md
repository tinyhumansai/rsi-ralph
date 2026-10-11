# Plan: Ship the first TinyBus module release

> **Historical.** This plan is complete and kept as a record of how the first
> module release landed. The module was later removed; see
> [`../specs/crate-split.md`](../specs/crate-split.md). Do not update this
> file.

1. Land the `lineage` feature area: the `Append` and `Head` members over
   TinyBus module ABI v1.
2. Export the lineage behavior through the module ABI and wire the ledger
   state into the service.
3. Verify a locally built `cdylib` through the real TinyBus dynamic loader
   (`verify_module`), then a tagged release archive (`verify_github_release`).
4. Replace TinyBus host bundles with tagged `tinyrsi` module archives for
   every platform the release matrix covers.

## Status

Delivered for the Ubuntu/macOS/Windows stable matrix in the current release
workflow; step 4 lands with the first tagged release of this repository.
