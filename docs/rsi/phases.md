# RSI phase status and remaining acceptance

This page separates implemented library components from a deployed improvement
loop and from evidence of improvement. It tracks
[umbrella #1](https://github.com/tinyhumansai/tinyrsi/issues/1) and
[scoring #2](https://github.com/tinyhumansai/tinyrsi/issues/2).
The [framework](specs/rsi-framework.md) defines the epoch and ownership rules.

| Phase | Current status | Remaining proof of completion |
|---|---|---|
| P1 — Observe and record | Observer, dossier and durable archive implemented | Host epoch collection and restart/replay demonstration |
| P2 — Human-gated shadow loop | Evaluation, approval and memory ports implemented | Private-suite host runner, snapshot provider and live restoration |
| P3 — Verified decomposition | Runtime DAG and explicit verifier reports implemented | Actual completion gate and code/Lean suites |
| P4 — Creative retreat | Actual tinybackrooms engine integrated | Durable retreat memory and measured proposer ablation |
| P5 — Structural edits | Activation granularity mapping implemented | Host roster/replacement/restart execution and rollback |
| P6 — Ignition | Research objective documented | Preregistered experiment and independent results |

None of P2–P6 is complete merely because its API exists. The live arithmetic
smoke has passed through the real model-backed retreat and evaluation funnel;
it establishes integration only. It does not establish gains on private code,
Lean or research suites. A selected archive head records a design decision,
not evidence that a live coordinator applied that design.

## Phase trackers

- P1: https://github.com/tinyhumansai/tinyrsi/issues/3
- P2: https://github.com/tinyhumansai/tinyrsi/issues/4
- P3: https://github.com/tinyhumansai/tinyrsi/issues/5
- P4: https://github.com/tinyhumansai/tinyrsi/issues/6
- P5: https://github.com/tinyhumansai/tinyrsi/issues/7
- P6: https://github.com/tinyhumansai/tinyrsi/issues/8
- Language: https://github.com/tinyhumansai/tinyhivemind/issues/122

The typed-verifier follow-up extends the merged language issue #112.

## P1 — Read-only observation and durable evidence

Implemented components:

- `Observer::snapshot` reads incremental transcript references and retained
  episode phases. Its cursor advances after both reads succeed. It discards
  message text and private failure reasons; the separate reads are not atomic.
- `Dossier::build` deterministically aggregates host-supplied token usage,
  wasted turns, failed/open nodes and memory-health counters.
- `Archive` persists the language's exact `Record`, candidate `Package`, memory
  watermarks, typed proposal declarations, outcomes and paired evidence. Replay
  validates identity and ancestry; `prepare` preserves unscored candidates;
  `conclude` commits a candidate and its final outcome together.
- `Archive::revert` changes the selected design pointer to its accepted parent
  without deleting history. It does not restore a live runtime or memory store.

Remaining acceptance for the host integration:

1. Collect complete epoch windows from `Coordinator::subscribe`, the observer
   cursor and actual host turn/verifier callbacks without scheduling agents.
2. Persist the epoch boundary and redacted dossier so restart resumes without
   losing or counting the same measured turn twice.
3. Demonstrate manual grading, reopen/replay, rejected-history queries and
   one-step pointer revert in an offline host example with recorded evidence.
4. Keep every public artifact free of transcript bodies, private task text and
   oracle answers. Document storage locking and crash-recovery limitations.

The low-level coordinator observation API already exists. This follow-up should
integrate it rather than request another observation API in tinyhivemind.

## P2 — Human-gated shadow selection

`run_proposal` supplies class-scoped formalization, language guards, grounded
references, exact deduplication, prepared snapshots, screening, selection,
independent confirmation and an `ApprovalGate`. `Evaluator` is a host port;
its memory pinning and task execution are not production providers.

`compare` implements a raw-quality paired ±2SE heuristic. `QualityRubric` and
`JudgeCalibration` provide eval-owned checkpoint weighting; `score`, `compare_h`
and `AnchorLedger` provide normalized bootstrap scoring and anchor monitoring.
The host still owns suites, reference anchors, calibration data and health-rate
denominators. Monitoring advice does not automatically roll back a runtime.

Remaining acceptance:

1. Implement a host runner for context, memory and prompt proposals using
   private selection/confirmation/anchor suites, paired seeds and execution-time
   caps. Use normalized scoring for scientific claims.
2. Supply isolated pinned memory snapshots and check learning-entry
   preconditions. Shadow writes must never merge into incumbent memory.
3. Ask a human to approve the exact candidate identity after confirmation.
   Denial, host failure and persistence failure must leave the live incumbent
   unchanged.
4. Restore both package and memory on revert, and test interruption/restart
   between selection, approval and application.
5. Report family quality, bootstrap intervals, tokens, false claims, tamper and
   canary checks from actual runtime evidence.

Reuse configured deployment work in
[tinyhivemind #117](https://github.com/tinyhumansai/tinyhivemind/issues/117) and
identity/memory work in
[#119](https://github.com/tinyhumansai/tinyhivemind/issues/119). This phase owns
RSI isolation, selection and approval integration, not a second deployment API.

## P3 — Verified decomposition in a running hive

`Blueprint::new` validates a nonempty dependency DAG and rejects preverified
nodes. `ready` exposes nodes with verified dependencies; `report` consumes an
explicit host verifier outcome; `is_complete` requires every node verified.
Failed prerequisites revoke dependent passes. Proof, execution, schema, private
reference, rubric and preference are ordered; the latter two are weak evidence.

This state machine does not execute Lean, run tests or intercept
`complete_episode`. It cannot authenticate an arbitrary caller's claimed pass.

Remaining acceptance:

1. Bind reports to observed checker runs with node identity, transcript sequence,
   artifact identity, command and exit status, without exposing private oracles.
2. Integrate the completion gate already tracked in
   [tinyhivemind #100](https://github.com/tinyhumansai/tinyhivemind/issues/100).
   Exercise refusal before verification and acceptance only after required
   checks; use the same gate in hive and single-agent arms.
3. Add a pinned code family and a private Lean slice checked by `lake build`.
   Report eval-owned per-node credit, open/sorry nodes and verified nodes per
   token; self-created decomposition must not create scoring checkpoints.
4. Keep immutable verifier definitions separate from mutable runtime status.
   Raise the remaining language contract additions on
   [#112](https://github.com/tinyhumansai/tinyhivemind/issues/112).

Do not create a second generic completion-gate issue. The new work is the RSI
blueprint/verifier adapter and the curated evaluation suites around that gate.

## P4 — Creative retreat

`typed_retreat_request` constructs and validates the actual vendored
`tinybackrooms::GenerateRequest`; `generate_retreat_with_engine` runs its
proposal/critic conversations, formatter extraction and configured scorers.
The HTTP transport adapter remains available. Strict `Proposal` decoding counts
invalid returned items and retains no raw model output in public artifacts.

The request uses `critic_pair`, comparative/judge/originality scoring, ranked
selection and `project:rsi-<hive>/agent:retreat`. Public package projection and
redacted dossier evidence exclude protected host fields and raw rejected edits.
Deterministic tests run the actual engine with `ScriptedConversant`.

The opt-in `live_openrouter` example also exercised the real engine and model
backend: five model calls used 5,249 measured tokens and produced one validated
ranked proposal. Screening, selection and confirmation each used two paired
seeds and measured 108 incumbent / 130 candidate tokens. The arithmetic paired
gain was 1.0 with a raw ±2SE interval of [1.0, 1.0]. This small deterministic
smoke is not a calibrated improvement estimate. The human gate returned
`ApprovalDenied`; the baseline stayed selected and archive reopen passed.

Remaining acceptance:

1. Connect a persistent tinybackrooms `Memory` adapter scoped to the retreat,
   and demonstrate recall of labelled accepted/rejected outcomes after restart.
   Namespace routing alone does not provide durable memory.
2. Invoke the retreat at epoch boundaries and retain source mode, model, run
   identity, prediction and realized outcome without publishing private data.
3. Compare retreat proposals against an orchestrator-alone proposer on identical
   dossiers, proposal/evaluation budgets and seeds over independent runs.
4. Report proposals produced, schema/guard failures, screened proposals,
   confirmed acceptance share, verified gain and total generation/evaluation
   cost. Include at least twenty real proposals for scoring #2's worked example.
5. Separate arithmetic connectivity results from private-suite improvement
   claims. A successful model call or schema-valid item is not measured gain.

## P5 — Structural classes and safe activation

`apply_at` maps context/memory to the next turn, prompt to agent replacement,
roster to host management, and topology/policy/goal to a between-session boundary.
`CandidateOutcome` returns that boundary after approval. These are instructions
to the host, not an implementation of activation.

Remaining acceptance:

1. Execute approved replacement and roster edits through existing authorized
   host APIs while preserving required session and memory identity.
2. Restart a coordinator over the same durable storage between sessions for
   structural policy/topology changes. Test writer fencing and interrupted
   activation so two coordinators cannot operate the same active design.
3. Persist application success/failure separately from selection, reconcile it
   after restart, and restore the accepted parent package and memory on rollback.
4. Demonstrate structural proposals in shadow mode before scheduling human
   review. This implementation still requires approval for every live promotion;
   it does not grant autonomous live structural edits.

Reuse deployment/configuration work in
[#115](https://github.com/tinyhumansai/tinyhivemind/issues/115),
[#116](https://github.com/tinyhumansai/tinyhivemind/issues/116),
[#117](https://github.com/tinyhumansai/tinyhivemind/issues/117) and
[#118](https://github.com/tinyhumansai/tinyhivemind/issues/118), and identity
rules in [#119](https://github.com/tinyhumansai/tinyhivemind/issues/119).
Role scheduling and width experiments remain
[#102](https://github.com/tinyhumansai/tinyhivemind/issues/102), not this phase.

## P6 — Ignition experiment

No ignition result is established. An improved task solver is not evidence that
an evolved orchestrator improves other hives faster than its original ancestor.

Remaining acceptance:

1. Preregister original/evolved orchestrator identities, immutable judges,
   private suites, seed draws, total compute caps, margins and stopping rules.
2. Compare both orchestrators on identical starting hives and budgets over
   several independent trials. Keep selection, confirmation and anchor draws
   separate; match proposal generation as well as evaluation cost.
3. Report verified anchor improvement per token, elapsed time and total cost,
   with uncertainty, lineage and negative/inconclusive results.
4. Include memory-off and retreat-off ablations, contamination checks and
   human approval records. Keep the judge outside every editable surface.
5. Publish a reproducible experiment report. Call the result inconclusive when
   the predefined evidence cannot separate the arms; do not infer ignition from
   one successful proposal or arithmetic smoke run.

## Issue ownership and filing

All six phase tracking issues belong in tinyrsi for integration and experimental
acceptance. Their upstream blockers stay in the owning repositories: deployment,
completion and identity in tinyhivemind; snapshot transactions in tinymemory;
durable creative memory in tinybackrooms. Existing blockers must be linked,
not refiled under new titles.

The remaining language additions are an update on tinyhivemind #112: typed
immutable verifier definitions, a runtime evidence boundary and explicit
orchestrator approval constraints. Reusing its `Record` is already implemented.

Draft bodies are prepared separately for P1–P6 and the #112 update. They have not
been posted by the documentation task. Umbrella filing acceptance remains open
until phase issue URLs and the posted #112 update are linked from #1; drafts
alone do not satisfy it. Filing these follow-ups does not mark their phases
complete, and this work does not close the umbrella.
