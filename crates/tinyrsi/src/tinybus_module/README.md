# The `tinybus_module` adapter

This module turns the crate's lineage API into a loadable TinyBus module. It is
the only part of the crate a TinyBus host can reach; the improvement loop
(archive, orchestrator, host deployment) is not exposed on the bus.

| File | Responsibility |
| --- | --- |
| `mod.rs` | `LineageService`, `setup`, and the `module_export!` descriptor |
| `test.rs` | Served interface and member list match `tinyrsi-bus` |

## Design

`LineageService` holds one `Lineage` behind a `Mutex` and converts the crate's
public `append` and `head` functions into the typed `Append` and `Head` bus
methods. `setup` creates the service with an empty ledger, serves it at
`tinyrsi_bus::names::OBJECT_PATH` and claims `tinyrsi_bus::names::INTERFACE`.
Errors from `append` (`StepOutOfOrder`, `EmptyField`) are returned to the
caller as `tinybus::Error::failed` with the error's message.

`module_export!` declares the ABI v1 descriptor: one worker thread, provides
`ai.tinyhumans.tinyrsi.Lineage`, methods `Append` and `Head`, no signals, no
required or optional dependencies, and eager (not lazy) loading.

## Keeping names in step

The payload types come from `tinyrsi-bus`, so a rename is a compile error in
`crates/tinyrsi-bus/src/names/`, not a runtime surprise. The interface name
and member list are, however, also spelled as string literals in two macro
attributes (`#[tinybus::interface(name = ...)]` and `module_export!`), which
cannot take constants. The unit tests check that the interface attribute's
name equals `tinyrsi_bus::names::INTERFACE` and the generated member list
equals `tinyrsi_bus::names::METHODS`, in order. The `module_export!`
literals are not unit-tested; `crates/tinyrsi/examples/verify_module.rs`
covers them by loading a compiled `cdylib` through TinyBus's dynamic loader,
waiting for the interface name to be claimed, and calling `Head` and
`Append` over a real in-memory bus. `verify_github_release.rs` does the same
for a downloaded release asset. A change to any name must update all three
places together.

## Operational constraints

- **State is per load and in memory.** Restarting the host or reloading the
  module starts a new ledger at step 0. Nothing is written to disk.
- **No ownership across the boundary.** Handlers must not retain Rust-owned
  data across the bus; every payload is owned, `serde`-derived data.
- **Contract versioning.** A host binds when the module's
  `CONTRACT_VERSION` has the same major and an equal or newer minor
  (`tinyrsi_bus::is_compatible`). Bump the major for an incompatible payload
  change or a removed or renamed member, and the minor for an added member
  or optional field (`crates/tinyrsi-bus/src/version/`).
