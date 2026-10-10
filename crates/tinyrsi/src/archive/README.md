# Durable archive

`Archive::open(path)` opens a JSON archive file and validates every event before
exposing any history. `record` constructs an exact language record; `append`
verifies an existing record against its package. The founding record must be
accepted with no parent and no patch operations. Each later record names an
earlier parent and passes the pinned language patch guard against that parent.
`prepare` persists an unscored candidate and pinned watermarks before host
evaluation. Prepared packages survive host failure without fabricated grades
and remain accessible by content identity; a later evaluated record must match
that snapshot. Accepted candidates must descend from the active head. Rejected
candidates remain available through `rejected` and never activate themselves.

`revert` records an activation of the active head's immediate accepted parent.
It preserves all candidates and survives reopening.
`declare_proposal` commits the exact typed prediction, risk and effort before
evaluation, retaining host-private provenance across restarts.
`record_proposal` retains failed attempts without fabricating language records;
invalid patches may be logged here. These events contain public sequence references, not transcripts.
`record_evaluation` persists the exact paired host outcomes separately from
summary objectives. It checks matching task/family/seed keys, uniqueness, finite
quality in the allowed range and candidate identity syntax. New evidence pins
the budget, statistical policy and comparison together; replay recomputes the
comparison and rejects a mismatched summary. Older standalone evidence may omit
all three fields. Evidence may precede its
candidate record so an interrupted approval still retains evaluation results.

The host owns learning transactions. The archive checks a learning operation's
read watermark against its parent and requires a later result watermark. Other
retained identities cannot silently advance their watermark. The archive never
opens or mutates memory stores and cannot verify host entry preconditions.

An adjacent `.lock` file excludes other writers. Normal drop releases it; after
a process crash the operator must confirm no writer remains before removing a
stale lock. A same-directory `.pending` file is synced before atomic replacement;
the containing directory is synced after replacement on Unix. A crash before
replacement leaves the previous complete history. The complete JSON snapshot is replayed on
restart; malformed, partial, duplicate or forged histories are rejected. Writes
clone and validate state before touching storage. A directory-sync error after
replacement reports uncertain durability while retaining the committed state
in memory. Storage must support atomic file replacement; Unix storage must
also support directory syncing.

JSON storage is trusted host state. Replay detects broken identities and invalid
structure; records are not externally signed, so a valid edit to finite objective
scores or rationale cannot be authenticated against the original host evidence.

`conclude` commits an evaluated candidate and its final proposal outcome in one
snapshot replacement, preventing an accepted audit event without its record.
On non-Unix platforms the standard library cannot sync the directory handle;
the candidate file is synced and atomically replaced, but directory-entry
durability after power loss depends on the underlying filesystem.

`replay.rs` owns event validation and folding. `mod.rs` owns exclusive storage
and the public API; this split keeps integrity rules distinct from IO handling.

Normalized H evidence uses dedicated `NormalizedEvaluationEvent` and
`NormalizedApprovalEvent` events rather than converting a raw comparison. Each
stage binds exact incumbent/candidate IDs, proposal declaration, all frozen
suite fingerprints, budget, validated policy specification, rates, paired runs,
and deterministic bootstrap output. Replay recomputes `compare_h`, validates
stage order and frozen settings, and rejects altered evidence. A normalized
accepted Record also needs independent positive confirmation, explicit approval,
and an exact measured `normalized_h` objective. Denied or unavailable approval
never advances the head. Archives containing these events write schema version
two; version one histories remain readable. This validates internal consistency
of trusted host telemetry, not external authenticity of unsigned host evidence.

Normalized proposals additionally persist `NormalizedDeclarationEvent` before
memory pinning, preparation, or evaluator callbacks. It freezes the incumbent,
base validated H policy, exact proposal prediction, hard budget, and all three
private suite fingerprints. The effective margin is derived from that base
policy and the declared prediction; stage evidence cannot choose a later policy.
Declarations in one epoch cannot contradict parent, policy, budget, or suites.
A matching prepared/final candidate or previous attempted-proposal output blocks
retroactive declaration. Schema 2 also covers a declaration-only crash prefix.

A candidate belonging to a normalized declaration cannot bypass evaluation by
calling `record` directly, including when it omits preparation altogether.
Replay checks the actual package against the declared patch and requires exact
normalized evidence and approval before accepting it. Legacy unrelated lineage
records and schema 1 histories retain their existing behavior. A crash prefix
remains inspectable and replayable; resuming an incomplete normalized run is a
host decision and is not synthesized by this archive.
