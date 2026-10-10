# Durable retreat recall cache

`DurableRetreatMemory` implements the real tinybackrooms `Memory` trait. Open it
with one hive identity, storage limits and host-protected strings; pass an `Arc`
to `Engine::with_memory`. A stable OS lock rejects a second writer, including
another adapter in the same process. Dropping the adapter releases the lock.

Discovery and host outcomes have independent oldest-first rolling windows. Each
window retains at most `MemoryLimits.max_entries` and `max_bytes`: discovery
counts UTF-8 text bytes; outcomes count their serialized entry bytes. Adding
valid entries evicts the oldest entries until both bounds hold, so repeated
epochs continue operating after the cache fills. Recall returns the recent
retained suffix in insertion order and does not perform semantic retrieval.
This cache is not authoritative lineage or an audit ledger: retain full outcomes
and attempt evidence in `Archive` and `EpochJournal`.

Every incoming entry is checked before eviction, including earlier batch items
that would fall outside the retained window. Contaminated values reject the
whole transaction. An individually oversized discovery entry or outcome is
rejected instead of silently dropping it. Corrupt, wrong-namespace, oversized
or contaminated stored history fails closed on reopen; reopening does not
silently trim data written with incompatible smaller limits.

Stores replace a versioned JSON snapshot atomically and sync it. Unix also syncs
the parent directory; other platforms' power-loss directory durability follows
their filesystem behavior. Pre-rename failures preserve process-local state;
after rename, local state follows the visible snapshot even if directory sync
reports failure. Symlink snapshots and lock files are rejected.

Engine selection stores discovery proposals, not RSI approvals. Explicit
`record_outcome` calls maintain a separate window of host approved/rejected
candidate identities. These labels are never presented as creative discovery
entries. Repeating an identical outcome is idempotent while its epoch/candidate
pair remains retained; contradictory classification is rejected within that
window. After eviction, cache-local deduplication no longer remembers the pair;
authoritative history and replay policy belong to the archive/journal.

The caller must supply all protected strings and retain the adapter throughout
the engine lifetime. Protected-value checks examine raw text and decoded JSON
keys and strings, including escaped values. Engine memory failures become
warnings; a host requiring durability must inspect those warnings before
continuing. Rolling eviction of valid old entries does not produce a warning.
