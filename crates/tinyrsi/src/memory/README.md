# Immutable hive memory

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

The store checks all images on each transaction, so canaries detected on reopen
fail closed. It is a local host implementation, not a CortexDB adapter: callers
can implement the `MemorySnapshot` boundary for other engines, preserving exact
snapshot semantics. Run-local evaluation changes never merge back into this
store. Files are host-private; do not place them in a proposal model's workspace.
