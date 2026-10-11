# Penalizing bad hive runs caused by patches

Status: Accepted. Plan: [`../plans/hive-penalties.md`](../plans/hive-penalties.md).

## Problem

On the hive path a patch that makes a run bad is never really penalized.

- **Crashes are blamed on the host.** A candidate that crashes the hive, times
  out, overruns its token reservation or lowers to a hive the evaluator cannot
  run surfaces as `Error::HostFailed`, and the orchestrator records
  `HOST_FAILURE`. `HOST_FAILURE` is not a terminal rejection, so proposing the
  same patch again is `DUP_UNRESOLVED` and is evaluated again. No evidence of
  the crash is kept and the loop trace reports `failures: 0`.
- **Magnitude is dropped.** Retreat sees only epoch, class, a coarse reason and
  an operation count, so a crash, a confident regression and a flat result look
  the same to the proposer.
- **Memory is write-only.** Retreat memory stores only approved or rejected
  labels and nothing reads them back.
- **No enforcement.** Deduplication is exact-patch only: a trivially different
  but equally bad patch spends a full Screen budget again.
- **Rollback forgives.** `DeploymentStore::rollback` leaves the bad record
  counted as a plain acceptance and nothing learns from it.
- **Anchor decline is ignored.** `AnchorLedger` signals a sustained regression
  on the frozen anchors, but nothing acts on it; rollback is manual only.

## Goals

1. **Attribute:** tell a run broken by the patch from one broken by the host.
2. **Remember:** keep a strike ledger derived from durable archive events.
3. **Enforce:** a cooldown that stops a struck patch shape before any
   evaluation spend.
4. **Teach:** coarse, redacted outcome feedback to the proposal generator.
5. **Revert:** roll the deployed design back automatically when its frozen
   anchors regress.

## Non-goals

- A failure term in H or `ScorePenalties`. The scoring policy, approval gates
  and private suites are frozen host surfaces (`ROADMAP.md`), and none is
  needed: a candidate that fails runs its incumbent completed is already
  rejected `HARMFUL` and struck, so a failure term would only re-penalize runs
  that can never be accepted.
- Tamper measurement on the hive path. In private trials the host `Producer`
  builds every verification binding and the agent supplies only content, so
  there is no tamper vector to measure.
- Acting on a widening selection–anchor gap alone, rolling back past the
  immediate parent to a best-anchor ancestor, or freezing the responsible
  patch class.
- The script loop, which already rejects a candidate with failed seeds.
- Relabelling any recorded verdict. A revoked acceptance stays `Accepted`.

## Behavior

### 1. Attribution

`tinyrsi-types` gains `CandidateFailureKind` (`crash`, `timeout`,
`over_budget`, `unsupported`) and `CandidateFailure { kind, failed_runs, runs }`.
`tinyrsi::Error` and `tinyrsi_score::Error` gain
`CandidateFailed(CandidateFailure)`, with message
`candidate failed runs the incumbent completed`; the `From` mapping keeps it.

`CoordinatorEvaluator` tags every job with its arm and attributes failures:

| Situation | Result |
| --- | --- |
| The incumbent's lowered hive is unsupported, or any incumbent job fails | `HostFailed` (unchanged) |
| The candidate's lowered hive fails `validate` after the incumbent's passed | `CandidateFailed`, `unsupported` |
| A candidate request cannot be reserved within the budget, or names a seat the candidate lacks, after the incumbent's requests were | `CandidateFailed`, `over_budget` or `unsupported` |
| The incumbent arm completes and a candidate provider call fails (transport, status, credentials) or a job task panics | `HostFailed`: an outage is not evidence against the patch |
| The incumbent arm completes and a candidate job then fails otherwise | `CandidateFailed`: `timeout` (provider timeout), `over_budget` (reply over its reservation), otherwise `crash` |

The incumbent arm runs to completion first; only then do candidate jobs run.
Every candidate job runs, so `failed_runs` counts every failed candidate run
of the draw and `runs` the candidate runs attempted.

`ReasonCode::Harmful` (`HARMFUL`) means the candidate failed runs the incumbent
completed on the same draws. Both `run_proposal_h` and the legacy
`run_proposal` map `CandidateFailed` to it at any stage; every other evaluator
failure stays `HOST_FAILURE`. Before stopping, the orchestrator appends a
`CandidateFailureEvent { candidate_id, epoch, stage, kind, failed_runs, runs }`
archive event. Replay requires a prepared or recorded candidate, a known stage
and `1 <= failed_runs <= runs`.

`HARMFUL` is a terminal rejection: a later exact repeat is `DUP_REJECTED`.
`record_epoch` emits `measured` for a failed candidate with `mean: null`,
`failures: failed_runs` and `seeds: runs`, even when no stage completed. The
trace schema does not change.

### 2. Strike ledger

`Archive::penalties()` projects the event history into a `PenaltyReport`, in
the style of `proposal_dispositions`: pure, deterministic, read-only. Each
`Strike` holds a `PatchShape`, an epoch, a `StrikeKind` and, for proposal
stops, the stop's index in `Archive::proposals()`.

`PatchShape` is the patch class plus the sorted, deduplicated
`(operation tag, target)` pairs, ignoring literal values. Targets are the JSON
pointer, the document or prompt path, `path#item` for context items and
`identity#entry` for learning. Two patches that edit the same targets with
different text have the same shape.

| Strike | Source |
| --- | --- |
| `Harmful` | A proposal stop with reason `HARMFUL` |
| `Regressed` | A `NO_GAIN` or `NOT_REPLICATED` stop whose last matching normalized comparison (same epoch and patch) has an upper bound below zero |
| `Revoked` | A `revocation` event: an accepted design the host rolled back |

`HOST_FAILURE`, `INCONCLUSIVE`, `APPROVAL_DENIED`, `DUPLICATE`, `COOLDOWN` and
prefilter rejections are not strikes: none is evidence the patch is bad.
Regression uses normalized (`ScoreComparison`) evidence only; the legacy raw
funnel produces `Harmful` and `Revoked` strikes but never `Regressed`.

### 3. Cooldown

`PenaltyPolicy { strikes, window_epochs }` sits in `Epoch` next to the budget,
default two strikes within three epochs. A shape is cooled at epoch `E` when
its strikes from epochs `e <= E` with `E - e < window_epochs` include a
`Harmful` or `Revoked` strike, or number at least `strikes`. Zero `strikes` or
zero `window_epochs` disables cooldown. The policy, default or not, is part of
the epoch's policy fingerprint, so it cannot change between generation and
evaluation. Journals prepared before penalties existed no longer match and
must be prepared again; no compatibility with them is kept.

The shared prefilter checks cooldown after the exact-duplicate check: an exact
repeat stays `DUPLICATE`, a same-shape patch stops as `ReasonCode::Cooldown`
(`COOLDOWN`) before declaration, memory pinning or any evaluator call.
`COOLDOWN` is not terminal, so a duplicate of a cooled patch is
`DUP_UNRESOLVED`, and a cooled shape becomes eligible again once its strikes
leave the window.

### 4. Revocation

`Archive::revoke(epoch, reason)` appends
`revocation { revocation: { candidate_id, epoch, reason } }` for the head and
the `activation` of its accepted parent in one transaction, and returns the
parent. `RevocationReason` is `manual` or `anchor_regression`;
`Archive::revocations()` lists the events in order. `DeploymentStore::rollback`
calls it with the checkpoint's epoch and `manual` instead of `revert`. The
record keeps `Verdict::Accepted`; the ledger counts a `Revoked` strike against
its patch shape whatever the reason. `Archive::revert` is unchanged.

### 5. Anchor regression rollback

`DeploymentStore::enforce_anchor(signal, archive, backend)` acts on the
`AnchorSignal` the host's `AnchorLedger` returned for the epoch and reports
whether it rolled back:

| Signal | Action |
| --- | --- |
| `regression` | Roll back exactly as `rollback` does, recording an `anchor_regression` revocation; the shape takes a `Revoked` strike and is cooled |
| `widening_gap` only | Nothing: advice for the operator |
| Neither | Nothing |

A regression means the deployed design is measurably worse on frozen
references nothing can optimize against. A widening gap means the selection
suite is drifting away from the anchors, which may be overfitting or a genuine
gain the anchors do not cover; the anchors have not fallen, so there is no
evidence the deployed design is worse. Rollback errors propagate unchanged:
`NoParent` for a parentless head and `HostFailed` for pending work or a
deployment that diverged from selection. The host calls it at the same
quiescent boundary as `activate` and `rollback`.

### 6. Generator feedback

Prior-idea seeds gain `"outcome"`: `crashed` (`HARMFUL`), `regressed` (a
`Regressed` strike), `flat` (other `NO_GAIN`, `NOT_REPLICATED`,
`INCONCLUSIVE`), `cooled` (`COOLDOWN`), `denied` (`APPROVAL_DENIED`), or `null`.
`tinyrsi-evolve` exposes the seed as `PriorIdea`, whose fields stay private so
nothing but epoch, class, whitelisted reason, operation count and outcome can
be sent. `prepare_retreat_epoch` lists struck attempts first, otherwise in
archive order. No delta value, interval, patch body, rationale or private task
data reaches the model.

`RetreatOutcomeStatus` gains `harmful` and `regressed`. `RetreatMemoryOutcome`
gains an optional public `shape` (omitted when absent, so stored bytes for
existing outcomes do not change). `evaluate_retreat_epoch` records them; a
shape containing a protected string is dropped rather than stored. An earlier
`rejected` label for the same pair is accepted as already remembered. Recall
folds one `Avoid these patch shapes ...` line, built from `outcomes()`, into
the returned discovery text.

## Invariants

- Trace and retreat-memory byte forms of existing data are unchanged; only
  additive variants and optional fields appear. The archive `revocation` event
  carries its reason; earlier revocation bytes are not read.
- Every new `ReasonCode` variant is pinned in the serde test.
- No proposer-visible text carries a delta, interval, patch literal, rationale
  or private task content.
- Cooldown never spends evaluation budget and never rewrites a verdict.

## Acceptance criteria

- A candidate-only crash yields `HARMFUL`, not `HOST_FAILURE`.
- An incumbent crash still yields `HOST_FAILURE`.
- A repeated harmful patch is `DUP_REJECTED`.
- A same-shape patch is cooled with zero evaluator calls.
- A strike ages out of the window.
- A revocation counts as a strike and the record stays `Accepted`.
- An anchor regression rolls the deployment back, records an
  `anchor_regression` revocation and cools the shape; a widening gap alone
  changes nothing; rollback errors propagate.
- Prior-idea seeds carry an outcome and no private fields.
- Retreat memory round-trips the new statuses.
- `cargo run -p tinyrsi-examples --example hive_goal` shows a crashing patch
  rejected `HARMFUL` with `failures > 0` on its trace and its near-duplicate
  stopped `COOLDOWN` without evaluation.

## Open questions

None blocking. Class freezing and best-anchor-ancestor rollback remain
separate work; a failure term in H and hive-path tamper measurement are not
needed (see Non-goals).
