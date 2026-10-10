# tinyrsi-bus

Every type that crosses the tinyrsi module's `TinyBus` boundary, and the names
of the members that carry them.

The module ships as a loadable module so a host does not compile the
implementation: `crates/tinyrsi` is built as a `cdylib` and exports one object.
A host can load that binary but cannot `use` anything out of it, so the payload
vocabulary has to be published as an ordinary library. This is it.

| module   | what it holds                                                          |
| -------- | ---------------------------------------------------------------------- |
| `names`  | interface name, object path, one constant per member                    |
| `lineage`| the value vocabulary: the payloads the `Append` and `Head` members exchange |
| `version`| `CONTRACT_VERSION` and the bind rule a host applies to it               |

Two dependencies, both pure Rust: `serde` and `serde_json`.

## This crate sits underneath `tinyrsi`

`tinyrsi` **depends on this crate and re-exports all of it**. That direction
matters, and it is the opposite of the obvious one.

A *host* needs the payload types and needs nothing else: it loads the module and
makes calls, so it names `GradeEntry` and `LineageAppendRequest` but implements
no behavior and links no transport. Making it depend on the whole module crate —
and through it on `tinybus`, `tokio`, and the module SDK — to spell a payload
type would be the wrong shape.

The alternative, a parallel set of payload types for hosts, is worse: a
`GradeEntry` defined twice is two distinct types, with a conversion at every
call site that nothing checks. One definition, here, at the bottom.

Because the re-export is by module as well as by item, `tinyrsi::GradeEntry`,
`tinyrsi::names::OBJECT_PATH`, and `tinyrsi_bus::lineage::GradeEntry` all
resolve to the same items, not twins.

So: a module author depends on `tinyrsi` and gets behavior and vocabulary.
A host depends on `tinyrsi-bus` and gets vocabulary alone.

## What is deliberately absent

**No behavior.** `append` lives in `crates/tinyrsi`. A payload type describes
what a frame carries, not what the module does with it. The split is readable
off the path: a name here is data, a name there is an obligation.

**No transport.** This crate does not depend on `tinybus` and holds no
connection, client, or codec. A host already owns its connection — its reconnect
policy, its timeouts, its tracing — and the useful part is the vocabulary.

That is also structural, not just preference: `tinybus` is vendored as a
submodule whose manifest inherits fields from its own nested
`[workspace.package]`. Keeping the contract crate transport-free is what keeps
it down to two dependencies and what lets anything in the workspace — or outside
it — depend on it freely. CI asserts the dependency tree stays that way.

## Making a call

Arguments travel as a positional JSON array — `#[tinybus::interface]` decodes
them into a tuple — and the member name comes from `names`:

```rust,ignore
use tinyrsi_bus::{names, GradeEntry, LineageAppendRequest, Verdict};

let proxy = connection.proxy(names::INTERFACE, names::OBJECT_PATH, names::INTERFACE)?;
let entry = GradeEntry::new(0, "sha256:abc", 0.703, Verdict::Accepted, "baseline");
let reply = proxy
    .call(names::methods::APPEND, (LineageAppendRequest::new(entry),))
    .await?;
assert!(reply.recorded);
```

Nothing above is a string literal at a call site. Renaming the interface, the
path, or a member is therefore a compile error in every consumer rather than an
`UnknownMethod` discovered at runtime.

## Staying in step with the module

`names::METHODS` lists every member in dispatch order. `crates/tinyrsi` asserts
its served members against that list, so a method added to the interface without
an entry here fails that crate's tests rather than surfacing in a host.

## Versioning

`CONTRACT_VERSION` describes *this vocabulary*, not the package. Bump its major
component when a payload's wire form changes incompatibly or a member is removed
or renamed, and its minor component when a member or an optional field is added.
It is deliberately independent of the package version the release workflow owns,
which tracks the shipped artifact.

The payload tests pin the serde representation, because that representation is
the wire form: a host and a module that disagree about a field name fail at
runtime with a decode error, so the shape is asserted rather than assumed.
