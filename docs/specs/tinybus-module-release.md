# Spec: TinyBus module release

> **Superseded** by [crate split](crate-split.md) and
> [ADR 0002](../adr/0002-split-crates-drop-bus.md): the TinyBus module, its
> contract crate and the native release archives were removed. Kept as a
> record only.

The module ships as a native `cdylib` per platform, verified before release.

## Requirements

- The module provides `ai.tinyhumans.tinyrsi.Lineage.Append` and
  `ai.tinyhumans.tinyrsi.Lineage.Head` at
  `/ai/tinyhumans/rsi-ralph/Lineage`.
- The release archive `rsi-ralph-<version>-<platform>.<extension>` contains only
  this module's artifacts and `MODULE.md`; `checksum.toml` is published
  separately.
- The release verifier loads the x86_64 archive through TinyBus's GitHub
  release API, calls `Append` with a valid baseline entry, and reads it back
  with `Head` before declaring the release successful.
