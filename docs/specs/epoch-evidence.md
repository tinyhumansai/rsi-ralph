# Durable P1 epoch evidence

Owns the evidence-pump requirement in tinyrsi #3. A host journals measured turn
usage and observed verifier results before completing work. It checkpoints a
redacted observation and deterministic dossier in the same transaction as the
consumed turn identities and transcript cursor. It never edits a hive package.

Each invocation receives a durable monotonic turn ID before its model call.
Callbacks name that ID: identical usage/checker retries are idempotent;
conflicting measurements or mismatched seat/episode bindings fail. Private
prompts, artifacts, verifier output and oracle contents never enter this store.
Checker evidence is recorded before the controlled completion gate is called.

Completed or explicitly interrupted turns enter exactly one epoch. Open turns
block checkpointing. An interrupted turn retains any recorded usage and checks;
missing provider usage remains unknown, never zero-cost success. The host must
stop admission and join runners before checkpointing or marking interrupted
turns. A crash between an external response and its durable usage callback cannot
recover that response; its retained intent exposes the uncertainty.

Checkpointing requires consecutive epochs starting at one. Repeating a completed
epoch returns its saved receipt, without reading or consuming later evidence;
its memory-health configuration must match. New epochs atomically persist the
observation, dossier, cursor, terminal episode identities and consumed turn IDs.
Failed persistence leaves evidence available. Restart retains all receipts.

The host subscribes through `Coordinator::subscribe`, waits for committed changes
and chooses an epoch boundary after work finishes. The transcript and episode
reads remain separate upstream snapshots; callers establish quiescence rather
than assuming an atomic cross-API view. Dropped/coalesced notifications are safe
because cursor reads, not notification counts, identify evidence.

The controlled single-seat factory opts into a shared journal. Evaluation
factories remain isolated unless the host explicitly installs this collector.
Legacy factory collectors remain available. The offline example uses actual
coordinator turns, measured deterministic provider responses and real golden
checks across two epochs, a reopened store and duplicate reads. It makes no
scientific improvement claim.
