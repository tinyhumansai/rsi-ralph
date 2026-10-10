# Durable retreat memory

`DurableRetreatMemory` implements the real tinybackrooms `Memory` trait. Open it
with one hive identity, storage limits and host-protected strings; pass an `Arc`
to `Engine::with_memory`. A stable OS lock rejects a second writer, including
another adapter in the same process. Dropping the adapter releases the lock.

Stores replace a versioned JSON snapshot atomically, sync it, then sync its
parent directory. Pre-rename failures preserve process-local state; after rename,
local state follows the visible snapshot even if directory sync reports failure. Corrupt,
wrong-namespace, oversized or contaminated stored history fails closed.
Discovery entries are bounded by count and combined UTF-8 size. Recent-entry
recall is deterministic and does not perform semantic retrieval.

Engine selection stores discovery proposals, not RSI approvals. Explicit
`record_outcome` calls maintain a separate bounded list of host approved/rejected
candidate identities. These labels are never presented as creative discovery
entries. The caller must supply all protected strings and retain the adapter
throughout the engine lifetime. Engine memory failures become warnings; a host
requiring durability must inspect those warnings before continuing.

Protected-value checks examine raw text and decoded JSON keys and strings,
including escaped values. Symlink snapshots and lock files are rejected.
Repeating an identical host outcome is idempotent; changing its classification
for the same epoch and candidate is rejected.
