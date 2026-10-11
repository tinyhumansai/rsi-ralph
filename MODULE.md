# TinyRSI TinyBus Module

This package contains the native `tinyrsi` module for TinyBus module ABI
v1. Install only the archive matching the host operating system and
architecture.

## What the module serves

The module claims `ai.tinyhumans.tinyrsi.Lineage`, serves one object at
`/ai/tinyhumans/tinyrsi/Lineage`, and provides two methods. It emits no
signals and requires no other module.

| Method | Request | Response | Behavior |
| --- | --- | --- | --- |
| `Append` | `LineageAppendRequest { entry: GradeEntry }` | `LineageAppendResponse { recorded }` | Records one graded candidate. Fails if `entry.step` is not the ledger's current length, or if the snapshot address or rationale is empty. |
| `Head` | `LineageHeadRequest` | `LineageHeadResponse { entry }` | Returns the last entry whose verdict is `Accepted`, or `None` before the first acceptance. |

A `GradeEntry` carries `step` (zero-based, contiguous), `snapshot` (content
address of the candidate's environment), `grade` (an `f64` whose scale the
writer owns), `verdict` (`Accepted` or `Rejected`) and `rationale`.

Payload types, the interface name, the object path, the member names and
`CONTRACT_VERSION` are published as the `tinyrsi-bus` crate, so a host names
them from a library rather than by string literal and can check
`tinyrsi_bus::is_compatible` before binding.

## What it does not serve

- **The ledger is in memory.** Each module load starts an empty ledger at
  step 0. Entries are lost when the host restarts or reloads the module.
- **The improvement loop is not on the bus.** Observation, proposal
  generation, private evaluation, approval, the durable `Archive`, and
  deployment and rollback are Rust APIs in the `tinyrsi` crate (most behind
  its `host` feature), called directly by a host program. Nothing in this
  module runs them, and loop results are not copied into this ledger
  automatically. See
  [`docs/architecture.md`](https://github.com/tinyhumansai/tinyrsi/blob/main/docs/architecture.md)
  in the source repository.

## Installation

The archive contains one `.so`, `.dylib`, or `.dll` plus `modules.toml`. Keep
those files together when copying them into a TinyBus module directory. The
allowlist binds the native library filename to its SHA-256 digest so TinyBus can
reject a missing, renamed, or modified artifact before initialization.

The GitHub release also publishes `checksum.toml` as a separate asset. TinyBus
checks that manifest before downloading and extracting the selected platform
archive. Install directly from a tagged release with:

```sh
tinybus modules load-github \
  https://github.com/tinyhumansai/tinyrsi/releases/tag/v0.2.1 \
  tinyrsi-0.2.1-ubuntu-24.04-x86_64.tar.gz \
  <archive-sha256>
```

TinyBus modules are trusted in-process code. Install release artifacts only
from a trusted source and restart the host after replacing a loaded module.
