# One epoch, traced through code and disk

This page follows a single epoch through the live example and lists every file
it leaves behind. Read [architecture.md](../architecture.md) for the overview
and the [glossary](../glossary.md) for terms. Source paths are relative to
`crates/tinyrsi/`.

The traced path is `examples/complete_openrouter.rs`, function `worked`, plus
the helpers in `examples/complete/driver.rs`. Deployment and rollback are
traced through `examples/complete/controlled.rs`, which the same binary runs
instead of the worked epoch and experiment when `TINYRSI_CONTROLLED_ONLY=1`. Measured results from these runs are in
[complete-live.md](complete-live.md).

## Running it

```sh
TINYRSI_LIVE_OPENROUTER=1 \
OPENROUTER_API_KEY=... \
TINYRSI_LEAN=/path/to/lean \
TINYRSI_COMPLETE_DIR=target/rsi-complete-live \
cargo run -p tinyrsi --features host --example complete_openrouter
```

Without `TINYRSI_LIVE_OPENROUTER=1` the example prints a hint and exits. It
refuses an existing `TINYRSI_COMPLETE_DIR`, so every run starts from an empty
directory (default `target/rsi-complete-live`). Below, `<root>` is that
directory.

## Step by step

### 0. Set-up (before the epoch)

`main` builds the private suites (`fixtures::suites`, which also creates
`<root>/verifiers/` for Lean and process checks), opens
`Archive::open("<root>/worked-archive.json")` and writes the founding baseline
record with `fixtures::founding`. It opens `SnapshotMemoryStore` at
`<root>/hive-memory.json` and seeds the founding memory. It then writes
`preregistration.json` and `configuration-provenance.json` for the experiment
that runs after the worked epoch.

### 1. Execute and 2. Observe — `driver::observed`

1. `CoordinatorHost::sqlite(..., "<root>/hives.sqlite", factory, memory)`
   creates a coordinator backed by upstream SQLite storage. The runner factory
   is `ControlledRunnerFactory` with one public probe task.
2. `DeploymentStore::open("<root>/deployment.json")` then
   `store.activate(&archive, &mut host)` deploys the founding record. This is
   needed because step 3 refuses to generate unless the archive head is the
   deployed design.
3. The host sends one public message with `Coordinator::send_as_host` and
   drains it with `run_until_idle`. The runner records token usage
   (`factory.usage()`) and checker evidence (`factory.evidence()`) outside the
   model's reply.
4. `store.observe(coordinator)` reads the transcript and episode phases since
   the last cursor, drops message bodies, increments the epoch to 1 and saves
   the observation inside `deployment.json`.
5. `Dossier::build(epoch, &observation, &usage, &outcomes, MemoryHealth::default())`
   makes the public dossier. The example aborts if no node failed, because a
   proposal must cite a failure.

`driver::epoch` then freezes an `HEpoch`: patch class `Context`, the dossier's
transcript refs and failed nodes, a 16,384-token run budget, the scoring policy
from `fixtures::policy` and the evaluator's three suite fingerprints.

### 3. Ideate — `prepare_retreat_epoch`

`worked` opens `DurableRetreatMemory` at `<root>/retreat-memory.json` and
`EpochJournal::open("<root>/worked-epoch.json")`, then calls
`prepare_retreat_epoch` in `GenerationMode::Retreat` with a 3,000,000-token
generation cap and `select_top_k = 25`. Inside
(`src/epoch_driver/generation.rs`):

1. The input is fingerprinted. If the journal already has this epoch with the
   same fingerprint and model, the stored proposals are returned with no
   model call. A different fingerprint is an error.
2. A journal entry recording the intent is committed **before** any call.
3. A receipt sidecar is created. Every formatter call gets an ID before
   dispatch and a disposition after.
4. The tinybackrooms Engine runs through `ModelConversant`. Strictly decoded
   proposals and the meter are committed to the journal.

The example requires at least 20 typed proposals; otherwise it stops with the
evidence kept.

### 4–5. Evaluate, score, approve — `evaluate_retreat_epoch`

`evaluate_retreat_epoch(&mut journal, &mut archive, &epoch, evaluator,
&mut fixtures::Deny, &discovery)` loops over the prepared proposals. For each
one it sets `evaluating = true` in the journal, calls `run_proposal_h`, then
records an `EpochCandidateOutcome` and clears the flag. Inside `run_proposal_h`
(`src/orchestrator/normalized/mod.rs`), each surviving proposal produces, in
order: a declaration and a normalized declaration, a prepared candidate, one
normalized evaluation per completed stage, and (after confirmation) the
approval decision and a concluded record. All are archive events.

The worked run uses `fixtures::Deny`, which always answers no. So the best
possible outcome is `ApprovalDenied`, the archive head stays on the baseline,
and the example checks exactly that, including after reopening the archive.
It then writes `worked-report.json`.

### 6. Deploy and roll back — `controlled.rs`

The worked run never deploys a candidate. The controlled path does: it runs
one model-authored proposal through `run_proposal_h` twice, once with `Deny`
on `worked-archive.json` and once with `fixtures::ContextGate` on a second
archive, `approved-archive.json`. After acceptance, `deploy_and_rollback`
opens `DeploymentStore` at `activation.json` and `CoordinatorHost` at
`activation.sqlite`, calls `activate`, checks one live verified turn, calls
`rollback`, then reopens both and calls `recover`.

After the worked epoch, `run_experiment` executes 24 trials. Each trial
repeats steps 1–5 in its own `trial-NN/` directory with fresh files.

## Files written

All JSON stores below are **whole-file snapshots**: every write serializes the
full state, writes it to a temporary file in the same directory, fsyncs it and
renames it over the original (directory fsync on Unix). None of them is an
append-only JSONL log, so do not tail them. A crash leaves the last complete
snapshot.

| File under `<root>` | Writer | JSON shape | Lock and temp file |
| --- | --- | --- | --- |
| `worked-archive.json`, `approved-archive.json`, `trial-NN/archive.json` | `Archive` (`src/archive/`) | `{"version": 1 or 2, "events": [{"event": "candidate" \| "prepared" \| "activation" \| "proposal" \| "declaration" \| "evaluation" \| "normalized_declaration" \| "normalized_evaluation" \| "normalized_approval", ...}]}`; version 2 once any normalized event exists | `<file>.lock` created with `create_new` and deleted on drop (see caveat); temp `<file>.pending` |
| `deployment.json`, `activation.json` | `DeploymentStore` (`src/host/mod.rs`) | `{"version": 1, "checkpoint": {"epoch", "observations": {"<epoch>": Observation}, "transcript_cursor", "settled_episodes", "deployed_id", "pending": DeploymentIntent \| null}}` | `fs2` exclusive lock on `<file>.lock`, non-blocking (`ArchiveLocked` if held); temp `<file>.next`, removed on open |
| `worked-epoch.json`, `trial-NN/epoch.json` | `EpochJournal` (`src/epoch_driver/journal.rs`) | `{"version": 1, "entries": {"<epoch>": {"incumbent_id", "fingerprint", "prepared": PreparedRetreatEpoch, "meter", "invalid_items", "evaluating", "outcome": RetreatEpochOutcome, ...}}}` | `fs2` exclusive lock on `<stem>.epoch-lock` (extension replaced, e.g. `worked-epoch.epoch-lock`), non-blocking; random `tempfile` name |
| `worked-epoch.json.generation-<epoch>.json` | receipt store (`src/epoch_driver/receipts.rs`) | `GenerationAudit`: `{"version": 1, "epoch", "fingerprint", "mode", "calls": [{"id", "status", "items": [disposition]}]}`; no prompts or bodies | written under the journal's lock; random `tempfile` name; copy together with the journal |
| `retreat-memory.json`, `trial-NN/retreat-memory.json` | `DurableRetreatMemory` (`src/retreat_memory/`) | `{"version", "namespace", "discovery": [text], "outcomes": [{"epoch", "candidate_id", "status"}]}` | `fs2` exclusive lock on `retreat-memory.lock` (extension replaced), non-blocking; random `tempfile` name |
| `hive-memory.json`, `trial-NN/hive-memory.json` | `SnapshotMemoryStore` (`src/memory/`) | `{"version": 1, "images": {key: {"namespace", "watermark", "entries"}}, "active": {identity: watermark}, "namespaces": {identity: namespace}}` | `fs2` lock on `<file>.lock` taken per transaction: shared for reads, exclusive for writes, **blocking**; random `tempfile` name |
| `hives.sqlite`, `activation.sqlite` | upstream `SqliteStorage` via `CoordinatorHost` | SQLite coordinator state and transcripts | SQLite's own locking; a newly built coordinator fences older handles |
| `preregistration.json`, `configuration-provenance.json`, `worked-report.json`, `controlled-report.json`, `experiment-report.json` | the example itself (`std::fs::write`) | pretty JSON reports | no lock, not atomic; written once |
| `verifiers/` | `fixtures::suites` | checker workspace for Lean and process checks | host-owned; must not change during a run |

`EvidenceJournal` (`src/evidence_journal/`) is not used by this example; the
offline `observe_epochs` example uses it at `target/rsi-observe-epochs/evidence.json`.
Its file is `{"version", "next_turn", "turns", "epochs", "consumed", "cursor",
"settled"}`, guarded by an `fs2` exclusive lock on `<file>.lock` and written
through `<file>.next` like `DeploymentStore`.

### Lock caveats

- **Archive locks are files, not OS locks.** `Archive::open` creates
  `<file>.lock` with `create_new` and deletes it when the `Archive` is
  dropped. If the process is killed, the lock file stays and every later open
  fails with `ArchiveLocked`. Confirm no writer is running, then delete the
  stale `.lock` by hand.
- **`fs2` locks are advisory and released by the OS.** `DeploymentStore`,
  `EpochJournal`, `DurableRetreatMemory` and `EvidenceJournal` use
  non-blocking exclusive locks held for the object's lifetime. A crashed
  process releases them, and leftover empty `.lock` files are harmless. A
  second opener in the same or another process gets `ArchiveLocked`.
- **`SnapshotMemoryStore` blocks.** It locks per transaction and waits for
  the lock instead of failing, so many readers (for example concurrent
  evaluator arms) can share it.
- **Two lock-name schemes.** `Archive`, `DeploymentStore`, `EvidenceJournal`
  and `SnapshotMemoryStore` append `.lock` to the full file name
  (`deployment.json.lock`). `EpochJournal` and `DurableRetreatMemory` replace
  the extension (`worked-epoch.epoch-lock`, `retreat-memory.lock`). Keep that
  in mind when copying or cleaning a run directory.
- **Copy a run while nothing holds it.** The snapshots are consistent per
  file, not across files. Stop the host before copying a directory, and copy
  each `EpochJournal` with its `.generation-<epoch>.json` sidecars.

## Reading the evidence afterwards

- `cargo run -p tinyrsi --features host --example proposal_metrics -- <journal> <archive> <epoch> <k>`
  projects a quiescent journal and archive into a selection-only report with
  precision at `k`. It opens both files, so stop the host first.
- `Archive::open` replays and validates the whole history, so opening an
  archive is itself an integrity check.
- `DeploymentStore::checkpoint()` shows `deployed_id` and any pending intent;
  a non-null `pending` means the next host start must call `recover`.
