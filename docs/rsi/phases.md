# RSI phase status and evidence

This page tracks [umbrella #1](https://github.com/tinyhumansai/tinyrsi/issues/1)
and [scoring #2](https://github.com/tinyhumansai/tinyrsi/issues/2). The
[framework](specs/rsi-framework.md) defines ownership; the
[host runtime specification](../specs/host-runtime.md) defines deployed behavior.
Implemented mechanisms and experimental efficacy are separate claims.

| Phase | Implemented mechanism | Evidence boundary |
| --- | --- | --- |
| P1 — Observe and record | Durable measured turn/checker callbacks, atomic redacted epoch dossiers, prepared snapshots and replay-validated archive | Host owns admission exclusion; interrupted responses before a durable callback remain explicitly unknown |
| P2 — Human-gated shadow loop | Normalized preregistration, isolated private suites, exact memory pinning, fresh confirmation and explicit approval | Private configuration and host checker trust remain external |
| P3 — Verified decomposition | Real Coordinator-backed verification/completion gate, executable/schema/reference/Lean checks and multi-seat DAG integration | Executing generated code requires a host sandbox; stock Lean is required for proof claims |
| P4 — Creative retreat | Actual tinybackrooms Engine, durable scoped memory, epoch journal and complete typed attempt capture | Failed calls and invalid formatting remain failures/counters |
| P5 — Structural edits | Durable selected/deployed separation, coordinator rebuild, fencing, compensation and exact parent rollback | Supplied factory is single-seat and rejects unsupported capabilities; custom factories own broader support |
| P6 — Ignition | Immutable preregistered original/evolved trial harness, matched caps, ablations and aggregate uncertainty | 24 live attempts, inconclusive; no accepted learned orchestrator or ignition claim |

The P3 contract and remaining integration sequence are documented in the
[verified decomposition specification](specs/verified-decomposition.md) and
[implementation plan](../plans/verified-decomposition.md).

## P1 — Durable observations and evidence

`DeploymentStore::observe` persists redacted transcript references and episode
phases together with the observation cursor and epoch. Reopening the store
retains those windows and avoids counting settled episodes again.
`Dossier::build` combines observations with measured host token/turn and verifier
callbacks; message bodies, failure text and private oracle contents are omitted.

`EvidenceJournal` adds the durable callback pump: invocation IDs are recorded
before dispatch, provider usage before artifact delivery, and checker evidence
before controlled completion. Atomic epoch receipts consume each measured turn
once together with the observation cursor and dossier. Duplicate callbacks and
repeated checkpoints survive restart without recounting. Open turns block
checkpointing; explicitly interrupted turns retain evidence and expose missing
usage. The offline `observe_epochs` example consumes coordinator notifications,
executes two actual checked turns across SQLite/journal reopen, and writes public
receipts under `target/rsi-observe-epochs/`. See the
[evidence contract](../specs/epoch-evidence.md) and
[implementation plan](../plans/epoch-evidence.md). Coordinator transcript/episode
reads remain separate snapshots, so hosts must stop admission at boundaries.

`Archive` uses the language's original `Record` and `Package`, validates
ancestry/content identities, and preserves prepared unscored states. It retains
proposals, raw paired outcomes, normalized policies, comparisons and approval
evidence. `conclude` commits the graded record and final outcome atomically.
Replay recomputes normalized comparisons and rejects approval/confirmation
bypasses. Archive pointer revert alone changes selection; live restoration uses
`DeploymentStore::rollback`.

## P2 — Private normalized selection

`run_proposal_h` writes normalized preregistration before memory pinning or any
evaluator callback. Frozen Screen, Selection and Confirmation fingerprints bind
the epoch; margins derive from the frozen host policy and exact proposal.
Language guards, read-only memory checks, grounded references, class boundaries
and duplicate suppression precede evaluation. Valid candidates are prepared
before model execution. Confirmation reuses neither tasks nor seeds from earlier
stages. The exact human decision is persisted; only valid positive confirmation
plus approval permits accepted normalized lineage.

`PrivateSuites` fingerprints actual private prompts, checker settings and seeds.
`CoordinatorEvaluator` runs each arm/task/seed with fresh coordinator storage,
owned pinned memory and a real `VerifiedRunner`. Provider usage is measured
outside model artifacts. `SnapshotMemoryStore` stores immutable local images,
checks learning preconditions/canaries, and keeps old versions for exact rollback.
It is not a CortexDB adapter, and shadow evaluation does not merge its writes
into live memory. `compare_h`, `QualityRubric` and `AnchorLedger` supply normalized
quality, verifier weighting, bootstrap uncertainty and independent anchor checks.

## P3 — Real verification and completion

`VerifierRegistry` freezes host checks. `VerifiedRunner` binds an artifact to its
active episode, seat, node and transcript references, then submits completion
only after required DAG nodes verify. Failed, forged or incomplete evidence
parks work without success credit. Tests exercise multi-seat dependencies with
actual coordinator settlement and host releases when parked dependencies become
ready. Self-created nodes do not become evaluation scoring checkpoints.

Process checks run a configured direct executable with bounded time/output and
a cleared environment. The host supplies any required sandbox. Schema and golden
references remain private. Lean checks append a restricted proof to a frozen
theorem and require a stock kernel axiom report; unavailable or failed checking
cannot receive simulated proof credit. Process workspace files remain an
immutable host trust boundary. No vendor code or OpenHuman completion API was
changed by these adapters.

## P4 — Durable real retreat attempts

`prepare_retreat_epoch` validates a genuinely persisted observation/dossier,
normalized policy, orchestrator instructions and generation seed before calls.
Engine mode uses the actual tinybackrooms Engine with `ModelConversant` and
`DurableRetreatMemory`; direct mode supplies the orchestrator-alone ablation.
Memory-off removes recall. Ranked items and every strictly decoded formatter
occurrence are captured without deduplicating attempts. Invalid outputs remain
counters; no fallback proposal is invented.

`EpochJournal` saves intent before external calls and prepared typed proposals
after generation. Completed generation/evaluation is reused after restart;
changed inputs reject reuse. Unknown interrupted effects fail closed rather than
repeating potentially spent calls. `evaluate_retreat_epoch` invokes the normalized
funnel synchronously, preserves classifications and stops after acceptance.
Durable retreat memory recalls labelled outcomes without publishing raw outputs
or private suite contents.

The earlier arithmetic [live smoke](live-smoke.md) remains connectivity evidence.
The complete live example adds real private-suite checking and the preregistered
experiment; its [measured report](complete-live.md) is inconclusive.

## P5 — Safe deployment and rollback

`DeploymentStore` journals exact from/to IDs before applying an accepted design.
`CoordinatorHost` validates/configures runners before restoring memory or fencing
a writer, then rebuilds on the same storage through existing upstream APIs.
Activation requires queues, parked turns, running reservations and unfinished
episodes to be quiescent; callers must stop admission. Selected archive state
and successfully deployed state remain distinct.

Failures leave a pending intent and block new activation/observation. Recovery
compensates by rebuilding the prior package with its exact memory watermarks.
First deployment without a prior snapshot retries the target. Rollback restores
the deployed record's accepted parent and completes selection/deployment
reconciliation after interruption. SQLite restart tests cover fencing, retained
logical sessions, queued-work reattachment and failed compensation paths.

The supplied `ControlledRunnerFactory` supports a bounded artifact-only single
seat with implemented prompt/context/recall/cap settings. Unsupported MCP, tools,
models or policies fail explicitly. Generic `RunnerFactory` implementations own
custom capability installation. Removed identities remain dormant; existing hive
metadata changes are rejected. This is full rebuild support, without a promise
of external model-process continuity.

## P6 — Bounded experiment and measured limits

`ExperimentProtocol` freezes original/evolved orchestrator identities, identical
starting solver hive, private fingerprints, family anchors, seed draws, caps,
stopping count and practical margin before outputs. `run_experiment` executes
Full, MemoryOff and RetreatOff arms with counterbalanced ordering, at least three
independent paired draws, two anchor seeds per draw and twenty total attempts.
The complete live example schedules 24 trial assignments after its worked run.

Every generation, retry, evaluation and initial/final anchor call contributes to
total cost. `BudgetedModel` reserves hard caps before dispatch. Returned evidence
is checked against preregistration; scores/gates and anchor comparisons are
recomputed. The endpoint is evolved minus original anchor H gain per 1,000 total
tokens, with a bootstrap over complete independent paired trials. Public reports
omit private per-task rows, prompts and oracle answers. Scripted evidence is
ProtocolOnly; missing evidence and failed gates prevent positive claims.

The [complete live measurements](complete-live.md) report 24 attempts with an
inconclusive verdict. The two frozen proposer variants were human-authored, so
an accepted learned orchestrator is not established. Even a supported
bounded improvement would not establish open-ended RSI or ignition; independent
replication remains necessary. See the [experiment contract](../specs/ignition.md).

## Trackers and upstream boundaries

P1–P6 trackers are [#3](https://github.com/tinyhumansai/tinyrsi/issues/3),
[#4](https://github.com/tinyhumansai/tinyrsi/issues/4),
[#5](https://github.com/tinyhumansai/tinyrsi/issues/5),
[#6](https://github.com/tinyhumansai/tinyrsi/issues/6),
[#7](https://github.com/tinyhumansai/tinyrsi/issues/7), and
[#8](https://github.com/tinyhumansai/tinyrsi/issues/8). Existing deployment,
identity and completion work remains owned by tinyhivemind; generalized memory
engines remain owned by their upstream projects. The remaining language contract
is tracked in [tinyhivemind #122](https://github.com/tinyhumansai/tinyhivemind/issues/122).
This repository implements the host integration through existing pinned APIs.
