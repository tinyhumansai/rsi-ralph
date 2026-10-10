# TinyRSI TinyBus Module

This package contains the native `tinyrsi` module for TinyBus module ABI
v1. Install only the archive matching the host operating system and
architecture.

The module claims `ai.tinyhumans.tinyrsi.Lineage`, serves the object at
`/ai/tinyhumans/tinyrsi/Lineage`, and provides the `Append` and `Head`
methods. `Append` records one graded candidate of an improvement lineage
(a `LineageAppendRequest` carrying a `GradeEntry`) and rejects an entry whose
step does not continue the ledger or whose snapshot address or rationale is
empty. `Head` answers which entry is the current incumbent: the last accepted
entry, or `None` before the first acceptance. Both payload types, the interface
name, the object path, and the member names are published as the `tinyrsi-bus`
crate, so a host names them from a library rather than by string literal.

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
