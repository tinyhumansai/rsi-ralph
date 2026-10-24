# Immutable hive memory

A hive's memory is part of the design being improved, so a candidate must be
evaluated with an exact memory image and a rollback must restore one exactly.
`SnapshotMemoryStore` provides that on a single local JSON file: every
version of every memory identity is kept as an immutable image named by a
*watermark*, and "live memory" is only a selection of watermarks.

| File | Responsibility |
| --- | --- |
| `mod.rs` | `SnapshotMemoryStore`, its transactions and the `MemorySnapshot` impl |
| `test.rs` | Locking, canaries, learning preconditions, rollback |

## Design

`SnapshotMemoryStore` stores immutable namespace/watermark images under a
cross-process file lock. Read-only transactions share the lock; writers wait for
exclusive ownership, so concurrent readers do not become failed agent turns.
Transactions stage and fsync a replacement in the same
directory. Active selection is a separate atomic operation; preparing a learning
patch never changes deployed contents. Old images remain available for exact
rollback and paired shadow evaluation. Each read returns owned maps.

Learning checks entry and watermark preconditions, declared writable hive
identities, language guards, contamination canaries and size limits. A new
identity begins empty at watermark zero; a declared fork copies its parent's
exact pinned image. Forking an untracked run identity fails rather than guessing
its contents. Retired identities leave their immutable images available for
rollback. An existing identity cannot move its underlying namespace implicitly.

## Public surface

| Method | Purpose |
| --- | --- |
| `open(path, forbidden, max_bytes)` | Open or create the store; `forbidden` are canary strings, `max_bytes` caps the file |
| `seed(package, watermarks, contents)` | Install founding images; an existing watermark cannot change |
| `read(package, watermarks)` | Owned entry maps for exact images |
| `pin(incumbent, patch)` | Apply the patch's learning operations as new images and return the candidate's watermarks, without changing live selection |
| `activate(package, watermarks)` | Atomically select existing images as live |
| `active_watermarks()` | The live selection, without memory text |
| `MemorySnapshot::restore` | Host-port impl used by `CoordinatorHost` for activation and compensation |

`CoordinatorEvaluator` calls `pin` from `Evaluator::pin_memory` to build each
candidate's memory before any trial runs.

## On-disk format

`{"version": 1, "images": {key: {"namespace", "watermark", "entries"}},
"active": {identity: watermark}, "namespaces": {identity: namespace}}`. The
lock is `<file>.lock`, taken per transaction with a blocking `fs2` lock
(shared for reads, exclusive for writes). Writes go through a random
same-directory temporary file and an atomic rename.

## Operational constraints

The store checks all images on each transaction, so canaries detected on reopen
fail closed. It is a local host implementation, not a CortexDB adapter: callers
can implement the `MemorySnapshot` boundary for other engines, preserving exact
snapshot semantics. Run-local evaluation changes never merge back into this
store. Files are host-private; do not place them in a proposal model's workspace.

- The whole store is one JSON file read on every transaction. It suits
  bounded experiments, not large production memories; `max_bytes` makes
  the limit explicit.
- Images are never garbage-collected. Rollback depends on that.
- Because locks block rather than fail, a stuck writer stalls every reader.
