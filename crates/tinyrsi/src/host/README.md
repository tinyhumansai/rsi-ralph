# Durable coordinator host

`DeploymentStore` separates a selected archive design from the package actually
installed in a coordinator. An accepted `Archive::head()` is evaluation evidence;
`checkpoint().deployed_id` records a completed runtime activation. All host
operations use the exact accepted record's package and memory watermarks.

| File | Responsibility |
| --- | --- |
| `mod.rs` | Atomic deployment checkpoint, activation, rollback and epoch observations |
| `types.rs` | Host ports and serialized intent/checkpoint definitions |
| `runtime.rs` | Real coordinator rebuild, retained sessions and memberships |
| `test.rs` | SQLite restart, configured runners, fencing and observations |
| `test_runtime.rs` | Membership changes, unsupported metadata and memory failures |
| `test_store.rs` | Journal failures and interrupted rollback recovery |

## Host lifecycle

1. Open `Archive`, `DeploymentStore`, and coordinator storage.
2. Construct `CoordinatorHost` with a `RunnerFactory` and `MemorySnapshot` owner.
3. Stop admission and call `recover` before scheduling any work. Recovery
   reattaches the recorded deployed configuration and fences older writers.
   With no pending activation, startup may retain queued/parked work under that
   exact configuration. Interrupted running turns are recorded by the upstream
   coordinator and never replayed automatically. Pending activation recovery
   still requires quiescence.
4. Submit work through the live coordinator and collect host usage and verifier
   outcomes. Call `observe` to save a redacted observation and advance the epoch.
   Observations remain in `checkpoint().observations` across restart.
5. Evaluate a proposal with the orchestrator and its approval gate. Acceptance
   changes selection, while rejection leaves selection and deployment unchanged.
6. Stop admission, finish eligible work and call `activate`. Queued deliveries,
   running reservations, parked agents and unfinished episodes block activation.
7. Call `rollback` at the same boundary to restore the deployed design's parent,
   restore its memory snapshot and move archive selection.

The caller owns admission control. A cloned `Coordinator` can submit new work;
all producers must participate in the host's activation exclusion mechanism.
Checking storage alone cannot prevent an unrelated producer from racing a check.

## Activation and recovery

The checkpoint records an intent containing exact `from` and `to` content IDs
before live mutation. A failure leaves the intent pending and blocks further
activation or observation. `recover` restores the previous deployment. When
first startup has no prior deployment, recovery retries the target instead of
claiming that unknown partial effects were undone. If rollback already changed
archive selection, recovery finishes deployment of that parent idempotently.

A coordinator rebuild validates and constructs all supplied runners before
restoring memory or claiming the new writer epoch. Failed validation or a
missing memory snapshot leaves the old writer unfenced. Once a new coordinator
claims storage, old coordinator handles are fenced permanently. Compensation
builds another fresh coordinator; it cannot resume an older fenced instance.

`MemorySnapshot::restore` must validate all identities and watermarks before
mutating live memory and restore them atomically. It runs for both activation
and compensation. `RunnerFactory::build` receives the retained logical session
ID and complete lowered seat configuration. The coordinator also retains its
stored session identity. `RunnerFactory::attach` receives the new coordinator
after fencing and before registration, allowing prepared verifier runners to
retain a weak handle. The backend holds the matching strong handle throughout
the deployed lifetime. This does not promise reuse of an external model's
process or connection.

## Configuration boundary

`RunnerFactory::validate` must reject every unsupported configuration: model,
tools, MCP, skills, memory behavior, routing, shared context and policies.
Receiving `LoweredHive` alone does not implement those settings. A host must
install them in its concrete runner and verify their use. Credentials stay in
the factory, outside portable packages and checkpoints.

All patch classes can use a complete coordinator rebuild. Membership additions
and removals use the upstream join/leave APIs. Removed seats and hives remain
in durable storage as dormant identities because the coordinator has no deletion
API. Existing hive names and descriptions cannot change through this adapter;
validation rejects such changes before writer fencing.

## Persistence

The `host` feature enables `CoordinatorHost::sqlite`, using upstream
`SqliteStorage` for transactional coordinator state and append-only transcripts.
`MemoryStorage` supports deterministic in-process integration tests. A custom
storage implementation must retain transcript rows and original retry payloads;
serializing `StoredState` alone omits those fields by design.

Deployment checkpoints use an OS advisory writer lock, a synced temporary file
in the same directory and atomic rename. Unix additionally syncs the containing
directory. Other platforms retain atomic replacement but power-loss directory
durability depends on the platform. Interrupted temporary files are discarded
under exclusive ownership. Storage is trusted host-private JSON, without an
external signature or authentication boundary.

Only transcript identifiers and episode phases enter observations. Message
bodies, model replies, private task descriptions and failure text are omitted.
