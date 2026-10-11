# Issue #1 implementation audit — 2026-10-11

Audited `main` at `1d0823f`, including merged PRs #9, #10 and #11, against
[umbrella #1](https://github.com/tinyhumansai/tinyrsi/issues/1),
[scoring #2](https://github.com/tinyhumansai/tinyrsi/issues/2) and P1–P6.
The issue bodies describe earlier implementation states. Most library and
bounded host mechanisms exist; full phase acceptance still needs work.

## Phase acceptance gaps

| Tracker | Implemented evidence | Work still needed |
| --- | --- | --- |
| [P1 #3](https://github.com/tinyhumansai/tinyrsi/issues/3) | Read-only observer, deterministic dossier, durable observations/cursor and replay-validated archive; restart tests exclude consumed transcript references and settled episodes | A durable usage/verifier callback pump and offline two-epoch demonstration proving no measured turn is lost or counted twice across restart. Observation checkpoints persist references and phases, while the example reads usage/checker evidence separately from in-memory factory buffers. |
| [P2 #4](https://github.com/tinyhumansai/tinyrsi/issues/4) | Frozen normalized policies, private suite fingerprints, isolated snapshot memory, fresh confirmation, exact approval and parent restoration | Curated private multi-family tasks, empirical family anchors and a production approval/provider configuration. The local snapshot store supplies bounded isolation/rollback, but CortexDB integration remains upstream. |
| [P3 #5](https://github.com/tinyhumansai/tinyrsi/issues/5) | Host-owned registry, real checker evidence, per-episode DAG and controlled completion, code sandbox and stock Lean tests | A curated pinned Lean project/slice checked by `lake build`, production sandbox configuration, and integration with upstream immutable typed goal-verifier declarations. The current Lean adapter checks restricted proof blocks directly; the demonstration is not that private project suite. |
| [P4 #6](https://github.com/tinyhumansai/tinyrsi/issues/6) | Actual Engine, durable scoped retreat memory, persisted epoch attempts, safe per-occurrence formatter receipts and matched ablation harness; fifty typed occurrences in the post-review worked run | Calibrated proposer ranking/outcome metrics and a scientifically informative ablation with curated tasks. Historical live runs retain invalid counts only; the published bounded comparison is inconclusive. |
| [P5 #7](https://github.com/tinyhumansai/tinyrsi/issues/7) | Durable deployment intents, SQLite fencing, recovery/compensation, selected/deployed separation and exact parent rollback | Authorized production adapters and demonstrations for every activation class. Current backend rebuilds all classes between sessions; it does not implement the requested turn-boundary context/memory path or live agent replacement. The supplied factory is single-seat and rejects unsupported capabilities. |
| [P6 #8](https://github.com/tinyhumansai/tinyrsi/issues/8) | Immutable protocol, paired draws, budget accounting, ablations, uncertainty and retained failed attempts | Obtain an accepted learned orchestrator, then preregister an independent original-versus-learned comparison after phase acceptance. Both published variants are human-authored. The 24-attempt result is inconclusive and cannot establish ignition. |

P0 documents and phase trackers exist. The requested language additions were
raised as [tinyhivemind #122](https://github.com/tinyhumansai/tinyhivemind/issues/122),
which remains open. No vendored language or memory engine changes belong in this
audit branch. The survey verifies linked identifiers and the Weco reported
results; its unlinked conceptual pointers still need source-specific review
before their claims are treated as empirical evidence.

## Remaining scoring implementation

The [acceptance map](scoring-acceptance.md) gives the exact existing test names
and obligations. Its unresolved mechanisms are:

- Versioned detailed host reject taxonomy and linking safe schema/repair rows
  to outcome/prior data, including accepted versus rejected duplicate history.
- Semantic grounding beyond reference existence; targeted replay checks that
  distinguish no effect from canary regression.
- Blinded, position-swapped pairwise proposal judgments, calibrated improvement
  priors, Bradley–Terry ranking and cost-aware successive halving.
- Outcome dataset metrics (Brier, rank correlation and precision at k), with
  explicit missing/censored labels and private-data boundaries.
- Preregistered sequential testing with valid alpha spending and restart limits.
- Persistent comparable anchor history connected to eligible ancestor rollback
  and class freezing; durable tamper halt/review and class-search controllers.
- Actual-store contamination quarantine, event cleanup and invalidation of every
  affected anchor snapshot, followed by clean remeasurement.
- Ordinal human judge calibration and expanded memory-health denominators,
  latency, growth and attribution to the original writer after reassignment.

This branch implements one bounded missing mechanism:
`geometric_family_mean` computes the weighted geometric family diagnostic from
the frozen scoring policy. Positive scores retain their values above/below the
reference; zero produces zero, and a negative family is explicitly undefined.
It preserves existing score serialization and promotion decisions.

## Evidence and next implementation boundary

Source evidence: [host journal](../../crates/tinyrsi/src/host/mod.rs),
[host rebuild adapter](../../crates/tinyrsi/src/host/runtime.rs),
[example evidence collection](../../crates/tinyrsi/examples/complete/driver.rs),
[verification contract](../../crates/tinyrsi/src/verification/README.md), and
[epoch receipt types](../../crates/tinyrsi/src/epoch_driver/types.rs).
The [phase map](phases.md) and [live receipts](complete-live.md) document bounded
integration results and scientific limitations.

The next self-contained implementation is P1's durable evidence pump: freeze
turn/check identities, journal callbacks before marking an epoch consumed, and
exercise duplicate reads and interruption with two offline epochs. Broader
production adapters and benchmark curation should follow their existing owners
rather than duplicate the already delivered host and snapshot infrastructure.

This audit does not close P1–P6 or label the live experiment an improvement.
