# Private coordinator evaluation

`PrivateTask` binds a host-private prompt, an immutable `HostVerifier`, a seat,
a task family, and explicit seeds. `PrivateSuites::freeze` separates screening,
selection, confirmation, and anchor draws. Task IDs and seeds cannot overlap
between stages. Fingerprints cover the actual prompts, checker configuration,
checker executable bytes, seeds, and quality protocol. Debug output contains
fingerprints rather than private task contents; these types have no serde
projection. Store private tasks only in host-controlled configuration. Process checker
workspaces and external references must remain immutable during trials; the
fingerprint binds the executable and command configuration, rather than every
file an arbitrary executable could read.

`CoordinatorEvaluator` implements `Evaluator` and `HEvaluator`. Each arm, task,
and seed gets a fresh in-memory coordinator and a real `VerifiedRunner`.
Only the host verifier can settle its artifact node. Provider token usage is
metered outside model-supplied artifacts. Failed checks earn no goal credit and
count as false claims; checker or model transport errors return an error rather
than a fabricated scored run. The artifact-only model has no coordination or
protected-state mutation capability, so tamper counts are zero for this adapter.
Quality follows `QualityRubric` with goal weight 0.5 and the actual verifier
strength. Process checks use the execution tier; golden references use the
held-out-reference tier. Proof credit requires the configured Lean checker.

The evaluator owns a Tokio runtime and must be constructed and called from a
synchronous thread. It rejects nested runtime use. Trials run in batches of at
most four independent episodes. Both arms' configuration, immutable memory
snapshots, executable identities, and conservative request budgets are checked
before any model call. Completed batches drain before an error is returned.

`SnapshotMemoryStore` supplies exact pinned memory maps. Recalling memory does
not activate or mutate a deployed snapshot. `evaluate_anchor_once` runs one
arm; `evaluate_anchor_seeds` selects a nonempty, unique subset of frozen anchor
seeds without changing the suite fingerprint. Record the selected seeds in the
experiment trial bindings. `suite_ids` exposes exactly the three stage names
required by `HEpoch`; `anchor_suite_id` exposes the independent anchor hash.

## Deployed runner

`ControlledRunnerFactory` supplies a concrete `CoordinatorHost` runner. It
accepts one declared seat and a frozen task/checker, attaches to the newly
fenced coordinator, and reads restored immutable memory at turn execution.
`evidence` returns host-verified outcomes and `usage` returns externally metered
turn usage. The model produces artifact bytes and cannot complete work itself.
The host retains logical session IDs across rebuilds; this stateless model
adapter does not claim provider-session continuity.

Supported configuration includes seat prompts, role descriptions, goals,
missions, shared context, permitted memory recall, and the implemented
coordinator/episode conduct limits. Evaluated and deployed schedulers use the
same conservative cap mapping. Explicit seat model names require
`with_model_name` to bind the externally configured model. MCP, tools, skills,
nonempty seat configuration, topology, referrals, responder-mode desks, and
unsupported core policies fail before memory restoration or model calls.
Use another validated `RunnerFactory` when those capabilities are required.

Model calls need a budget large enough for the conservative input reservation
plus the output cap. A `BudgetedModel` can additionally bound all calls across
an entire experiment, including anchors and failed attempts. Private checker
references never enter model requests or public fingerprints. Public hashes
are identifiers, not encryption; use high-entropy private tasks when resistance
to offline prompt guessing matters.

## Attributing failures

`CoordinatorEvaluator` tags every job by arm. It reserves both arms first,
then runs every incumbent job before any candidate job. An unsupported
incumbent hive, an unreservable incumbent request or any failed incumbent run
is the host's: the stage fails as before. When the incumbent completed but the
candidate did not, the stage returns `Error::CandidateFailed` with a
`CandidateFailure`: `unsupported` for a candidate hive that fails validation
or lacks a task's seat, `over_budget` for a request that cannot be reserved or
a reply over its reservation, `timeout` for a provider timeout, and `crash`
otherwise. Every candidate job still runs, so `failed_runs` counts all of the
draw's failed candidate runs and `runs` its candidate runs; anchor draws keep
stopping at the first failed batch. The orchestrator records the stop as
`HARMFUL` (see `docs/specs/hive-penalties.md`).
