# Host-driven hive improvement

Related: tinyhumansai/tinyrsi#1 and #2.

## Contract

Use the pinned tinyhivemind-lang Package, Patch and lineage Record unchanged.
Retain the legacy GradeEntry bus API for compatibility. The external host owns
agent execution, private tasks, memory transactions and human approval.

Observe a coordinator through its read-only transcript and episode APIs.
Build deterministic failure dossiers from host-supplied verifier and turn
usage outcomes. Track sequence references, seat costs, failed/open nodes and
memory-health counters. Do not mutate the coordinator.

Persist candidate packages, exact language records and eval evidence in an
exclusive, atomic file archive. Validate replay, identity, parent order and
patch ancestry before accepting a write. Record rejected/guarded proposals
separately when no valid language candidate exists. Revert selects an accepted
parent and persists an activation event without rewriting history.

Compare the incumbent and candidate on identical tasks, seeds and hard caps.
Require multiple seeds, finite data, no duplicates, no missing runs and no budget
overruns. Use paired mean gain and two standard errors with per-family and
health constraints; require independent confirmation before promotion. Screening
rejects clear losers before full evaluation. A human gate approves every live
promotion. Failed approval never advances the active head. Memory learning
requires an explicit host transaction and pinned watermarks; no implicit write.

The retreat receives the public failure dossier, incumbent and rejected archive,
and emits strict typed proposals with predictions, evidence, risks and effort.
Never send private tasks, oracle answers or constitution data to proposal models.
Formalize through the language apply/guard, reject duplicates and ungrounded
references, and append every outcome with a reason. Select one patch class per
epoch. Structural patches request between-session host activation.

## Failed-attempt recovery

Failed host attempts are also deduplicated. A prepared snapshot is retained for
audit, not treated as a resumable evaluation checkpoint. Retrying private
evaluation requires a separately specified recovery protocol that pins the
incumbent, epoch, suites, seeds, budget and policy, and distinguishes unobserved
infrastructure failures from partially observed trials. Reusing package and
watermarks alone does not establish those invariants. This API therefore does
not automatically retry `HostFailure` attempts; host recovery is part of P2.

## Limits

A local OpenRouter smoke suite can prove API connectivity, schema conformance,
paired execution and persistence. It cannot establish improvement on private
code/Lean/research suites, calibrate judges, or prove ignition. Those experiments
require curated host-owned suites and repeat runs.
