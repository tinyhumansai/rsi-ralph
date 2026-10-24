# Durable host runtime contract

RSI Ralph is the external host of a typed hive improvement loop. It records the
selected design separately from the deployed coordinator, evaluates against
immutable private checks, and requires explicit human approval. The
`rsi-ralph-host` crate (formerly the optional `host` feature) holds the concrete asynchronous coordinator/SQLite adapter and
private suite evaluator. Existing bus lineage methods remain compatible.

## Durable state boundaries

| Owner | Durable fact |
| --- | --- |
| `Archive` | Language packages/records, prepared states, proposal preregistration, paired outcomes, normalized comparisons and human decisions |
| `DeploymentStore` | Deployed content ID, exact pending from/to intent, observation cursor, settled-episode consumption and redacted epoch windows |
| `SnapshotMemoryStore` | Immutable namespace/watermark images and independently selected active memory |
| `DurableRetreatMemory` | Scoped discovery and labelled proposal outcomes |
| `EpochJournal` | Generation intent, strictly decoded attempts, accounting and completed epoch evaluation |
| `ExperimentProtocol` | Frozen arm identities, suites, draws, caps, scoring and stopping rule |

Files are trusted host-private state. Locks and atomic replacement detect
partial/corrupt state and exclude cooperating writers; they do not authenticate
an attacker who can rewrite complete files. Same-directory replacement fsyncs
the file; Unix also syncs the directory. Other platforms' power-loss directory
durability follows their filesystem behavior. Old memory snapshots and archive
history remain available for exact restoration.

## Observation and epoch execution

`DeploymentStore::observe` persists a redacted observation and its cursor/epoch
transactionally. Reads retain transcript identifiers and episode phases, never
message bodies or private failure reasons. Coordinator transcript and episode
reads are separate; the host chooses its collection boundary. Usage and checker
callbacks supply actual token counts and verifier outcomes to `Dossier::build`.

`prepare_retreat_epoch` requires the persisted observation and consistent dossier,
selected/deployed incumbent, frozen `HEpoch`, instruction string and generation
seed. Intent is durable before calls. Engine mode invokes actual tinybackrooms;
direct mode implements the orchestrator-alone arm. Normalized predictions are
requested before generation, never relabeled afterward. Every strictly decoded
formatter occurrence is retained as an attempt, including duplicate patches;
invalid outputs contribute counters without storing raw private text.

Reopening completed preparation/evaluation reuses recorded results without new
model calls, private draws or human decisions. Changed inputs reject reuse.
Interrupted unknown external effects fail closed. `evaluate_retreat_epoch` calls
the normalized funnel on a synchronous thread and stops after acceptance.
It does not implicitly activate the selected package.

## Evaluation and approval

`run_proposal_h` persists a normalized declaration before memory pinning or
execution. Language guards, protected verifier rules, read-only identities,
grounded references and duplicate suppression precede callbacks. `Archive::prepare`
preserves valid unscored candidates. Frozen stage fingerprints and base policy
bind all subsequent comparisons. Confirmation draws overlap neither earlier
tasks nor seeds. Replay recomputes comparisons, validates chronology and derives
the proposal-adjusted margin from the declared base policy.

`PrivateSuites` keeps task prompts and checker contents private and immutable.
`CoordinatorEvaluator` creates fresh in-memory coordinator storage and owned
pinned memory per task/seed/arm, with at most four concurrent episodes.
`VerifiedRunner` binds actual artifacts to host-owned nodes/seats/episodes and
permits completion only after required checks. Failed or forged evidence parks
work and earns no success credit. Multi-seat DAGs use the same completion gate;
the host releases parked seats when dependencies become ready.

Checks include exact private references, JSON Schema, bounded direct executable
runs, and restricted stock-Lean proof/kernel checking. Process execution is a
checker boundary, not an OS sandbox. The host must keep executable/workspace
inputs trusted and immutable, and sandbox generated programs as required.
Provider token usage is metered outside artifact submissions. Run-local memory
never merges into deployed memory. Canary and learning preconditions fail closed.

Human approval applies to the exact confirmed candidate ID and normalized
comparison. The explicit decision is persisted. Accepted normalized records
require positive confirmation and approval on replay. `conclude` atomically
stores the record with its final outcome. Denial, invalid evidence and host errors
do not promote a candidate. Neither archive selection nor approval alone changes
a live coordinator.

## Activation, restart and rollback

Before activation, the caller stops all admission sources, including cloned
coordinator handles. Queued deliveries, running reservations, parked agents and
unfinished episodes block changes. `DeploymentStore::activate` journals the exact
accepted from/to IDs before calling its `ActivationBackend`.

`CoordinatorHost` lowers the package, validates the complete configuration and
builds runners before memory restoration or writer fencing. `MemorySnapshot`
restores exact watermarks atomically; missing snapshots leave the old writer
unfenced. A fresh coordinator claims the existing storage writer epoch.
`RunnerFactory::attach` binds prepared runners before registration. Retained
logical session IDs and declared membership order are preserved. The backend
retains the exact coordinator handle for deferred verifier runners.

Once fenced, the older coordinator cannot be resumed. Errors keep a pending
intent and block further activation/observation. `recover` compensates using a
fresh coordinator with the previous package and memory snapshot. On initial
activation without a previous deployment, it retries the target instead of
claiming unknown effects were undone. Pending recovery requires quiescence.
Ordinary restart may reattach queued/parked work under the exact deployed design;
interrupted running turns are recorded by upstream and not automatically replayed.

`rollback` restores the deployed accepted record's parent package and pinned
memory, then reconciles archive selection and deployment durably. Restart after
selection has moved completes the intended parent deployment idempotently.
Selection diverging from deployment cannot silently substitute another parent.
SQLite supplies real transactional coordinator storage and retained transcripts.

## Supported runner boundary

The generic `RunnerFactory` must implement or reject all lowered model, tool,
MCP, skill, routing, context, policy and memory settings. Validation is mandatory
before mutation. Receiving the configuration is not proof of implementing it.
Credentials belong to the external factory, never portable packages.

The supplied `ControlledRunnerFactory` supports one declared seat and a frozen
artifact verification task. It installs supported prompt/role/context/goals,
pinned recall, declared model binding and conservative episode/conduct caps.
Evaluated and deployed schedulers share the same options mapping. Unsupported
MCP/tools/skills, nonempty seat configuration, topology/referrals, responder-mode
desks and core policies fail explicitly. Custom factories own broader capabilities.

All patch classes may use full rebuild at a safe boundary. Removed durable seats
and hives remain dormant because upstream has no deletion API. Existing hive
name/description changes are rejected. Logical sessions are preserved; external
provider-process continuity is not promised. This implementation uses pinned
upstream APIs without changing OpenHuman or any vendor code. Local immutable
memory is not a CortexDB implementation.

## Experiment evidence

The [bounded experiment contract](ignition.md) freezes original/evolved arms,
identical starting solver state, private draws, caps and stopping rules.
`BudgetedModel` reserves before dispatch across generation, retries, evaluation
and anchors. Full, MemoryOff and RetreatOff assignments produce validated
aggregate uncertainty over independent paired trials. Raw private task evidence
is not serializable into the public report. Scripted runs are ProtocolOnly.
The [complete live report](../rsi/complete-live.md) records the inconclusive
comparison and separate accepted deployment validation. No positive RSI or
ignition conclusion follows from these toy fixtures.
