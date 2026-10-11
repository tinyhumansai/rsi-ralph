# Durable measured epoch evidence

`EvidenceJournal` records controlled host invocations and commits redacted
observations/dossiers atomically. It retains callback history independently of
deployment state and the proposal archive. The host owns admission control.

| File | Responsibility |
| --- | --- |
| `mod.rs` | Turn callbacks, interruption and atomic epoch construction |
| `types.rs` | Redacted invocation and epoch receipts |
| `storage.rs` | Exclusive writer lock, atomic rename and durable writes |
| `validation.rs` | Identity, callback and derived-dossier replay checks |
| `test.rs` | Restart, duplicate callback, failure and corruption regressions |

Open one journal and share it through `Arc` with a
`ControlledRunnerFactory::with_evidence_journal`. The factory persists a turn ID
before calling the provider, provider-measured usage before validating/returning
the artifact, and checker evidence before updating verification/completion.
Failure to persist prevents the controlled runner from completing. An invocation
is marked finished before its outcome returns to the coordinator; finalization
errors still retain measured work rather than erase its cost.

At a host-owned epoch boundary, stop admission and join runners. Consume the
coordinator's `subscribe()` signal and call `checkpoint(epoch, coordinator,
memory_health)`. Notifications may coalesce: transcript sequence and turn IDs,
rather than notification counts, determine what enters the receipt. The dossier,
observation, consumed IDs, terminal episodes and cursor commit in one snapshot.
Repeating an epoch returns its saved receipt; changed memory health rejects
replay. A new epoch consumes only callbacks not already assigned to an epoch.

Open turns block checkpointing. After ensuring the prior runners cannot continue,
`interrupt_open_turns()` retains their evidence and marks the uncertain work.
`unmetered_turns` lists missing usage explicitly; it is not a zero-cost success.
Dossier usage counters include measured turns; unmetered IDs remain separate
instead of silently supplying invented token counts.
A process crash between a remote response and a durable callback cannot recover
that response. It leaves an intent for the host to reconcile without retrying
the potentially spent call. Usage/checker retries on an existing turn ID are
idempotent; changed data fails even after consumption. New invocations always
receive new IDs, including actual resumed work in the same episode.

The coordinator's transcript and episode APIs are separate reads. This journal
does not invent cross-API atomicity or prevent an unrelated producer from
submitting work. Host admission exclusion is required for a stable boundary.
Stored verifier evidence is host-owned; this is a persistence port, not another
grader or a model-accessible completion API. Private artifacts, prompts, oracle
answers and subprocess output are absent from serialized records.

Run `cargo run -p tinyrsi --features host --example observe_epochs` for two actual
offline turns and golden checks across a SQLite/journal restart. Public receipts
are written to `target/rsi-observe-epochs/epochs-public.json`. Running it again
reopens the two existing epochs without generating new work. The deterministic
provider supplies fixed usage accounting; this is regression evidence, not a
hosted-model efficiency experiment.
