# The `tinybus_module` adapter

This module turns the crate's public API into a TinyBus module. `LineageService`
converts the crate's public `append` and `head` functions into the typed `Append`
and `Head` bus methods, while `setup` registers its object and claims the
well-known interface name. The names and payload types it serves are spelled
here: they come from `tinyrsi-bus`, so a rename is a compile error in
`crates/tinyrsi-bus/src/names/`, not a runtime surprise.

The unit tests check two relationships: the served interface name equals
`tinyrsi_bus::names::INTERFACE`, and the served member list equals
`tinyrsi_bus::names::METHODS`; the unit tests check both relationships.

`crates/tinyrsi/examples/verify_module.rs` loads a compiled `cdylib` through
TinyBus's dynamic loader and calls the served methods over a real in-memory bus,
so a change to any of the three declarations together — here and in
`crates/tinyrsi-bus/src/names/`. They must not retain Rust-owned data across the
bus boundary; every payload is owned, `serde`-derived data.
