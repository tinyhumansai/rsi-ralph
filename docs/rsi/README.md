# RSI Ralph research and implementation

The improvement object is a typed tinyhivemind hive package and its exact pinned
memory image. An external host observes real work, generates proposals,
shadow-evaluates them against immutable private checks, and requests human
approval before live deployment. Selected lineage and deployed runtime state are
recorded separately.

- [Framework](specs/rsi-framework.md): epoch, ownership and phase boundaries.
- [Loop walkthrough](loop-walkthrough.md): one epoch traced through the live
  example and every file it writes, with lock caveats. See also the
  [architecture overview](../architecture.md) and [glossary](../glossary.md).
- [Phase status](phases.md): implemented P1–P6 mechanisms and experimental limits.
- [Remaining work audit](remaining-work.md): issue acceptance gaps after the
  merged host and scoring work, with the next implementation boundary.
- [Private numerical benchmarks](private-benchmarks.md): 48 independently checked
  agent-authored tasks and hard Euler controls with pinned published solutions.
- [Pinned Lean checks](pinned-lean.md): exact toolchain builds of 12 private
  finite mathematics reference proofs, with safe receipts.
- [Host runtime](../specs/host-runtime.md): durable deployment, compensation,
  exact memory restoration and the supported runner boundary.
- [Host implementation map](../plans/host-runtime.md): source and regression tests.
- [Durable epoch evidence](../specs/epoch-evidence.md): callback identities,
  atomic dossiers and the offline two-epoch demonstration.
- [Proposal disposition contract](../specs/proposal-dispositions.md): safe
  chronological accepted, rejected and unresolved exact-duplicate history.
- [Proposal outcome receipts](proposal-outcomes.md): source occurrences, frozen
  probability priors and selection-only descriptive metrics.
- [P1 offline receipts](p1-evidence.md): measured turns and observed checker
  passes retained across restart and repeated checkpoints.
- [Verified decomposition](specs/way-of-working.md): blueprint and verification model.
- [Scoring](specs/scoring.md): checkpoints, normalization, uncertainty and anchors.
- [Scoring acceptance map](scoring-acceptance.md): reject/decision test coverage
  and explicit remaining implementation obligations.
- [Recorded scoring example](scoring-worked-example.md): fifty retreat proposals,
  actual incumbent/candidate H intervals and the resulting rejection.
- [Implementation contract](specs/orchestrator.md): lineage and proposal gates.
- [Bounded experiment](../specs/ignition.md): preregistration, ablations and verdicts.
- [Experiment implementation](../plans/ignition.md).
- [Survey](rsi-survey.md): literature and citation-verification status.
- [OpenHuman substrate](openhuman-substrate.md): integration research; this work
  uses existing APIs without modifying the vendored OpenHuman engine.
- [Original implementation plan](../plans/2026-10-10-rsi-orchestrator.md).
- [Measured arithmetic smoke](live-smoke.md): earlier connectivity evidence.
- [Graph colouring runs](graph-coloring-results.md): live supervisors reach DSATUR
  (13 colours) but stall short of the optimum 6; why execution feedback, not
  ideas, is the bottleneck.
- [Diagnostics reruns](diagnostics-results.md): with `DIAG` feedback, Sonnet
  4.5 reaches the graph-colouring optimum in 3 epochs; gpt-4.1-mini still stalls.
- [Script loop roadmap](script-loop-roadmap.md): ranked next improvements with
  the failure each targets and how to measure it.
- [Script loop live run](script-loop-results.md): an LLM-supervised curve-fit
  loop cut RMSE 0.644 → 0.258 in three promotions, then stalled short of its
  0.06 target; why, and the ranked next improvements.

The `rsi-ralph-host` crate (formerly the `host` feature) now includes real coordinator evaluation, immutable local memory,
durable Engine epochs and attempts, human-gated activation, compensation and
rollback. The bounded experiment completed with an inconclusive 24-attempt
comparison. Neither the arithmetic smoke nor the implemented harness establishes
recursive ignition or general research-benchmark improvement.

See the [complete local integration results](complete-live.md) for all measured
failures, the separate accepted deployment validation and experimental limits.
