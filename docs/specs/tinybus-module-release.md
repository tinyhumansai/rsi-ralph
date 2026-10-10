# Spec: TinyBus module release

The module ships as a native `cdylib` per platform, verified before release.

## Requirements

- The module provides `ai.tinyhumans.tinyrsi.Lineage.Append` and
  `ai.tinyhumans.tinyrsi.Lineage.Head` at
  `/ai/tinyhumans/tinyrsi/Lineage`.
- The release archive `tinyrsi-<version>-<platform>.<extension>` contains only
  this module's artifacts and `MODULE.md`; `checksum.toml` is published
  separately.
- The release verifier loads the x86_64 archive through TinyBus's GitHub
  release API, calls `Append` with a valid baseline entry, and reads it back
  with `Head` before declaring the release successful.
