# Orchestrator

The orchestrator is the funnel every proposal goes through: cheap checks
first, then paired evaluation on private suites in three stages, then a human
decision. It writes only to the `Archive`. It never touches a live
coordinator, and an accepted design is not deployed until the host calls
`DeploymentStore::activate`.

| File | Responsibility |
| --- | --- |
| `mod.rs` | Raw-quality funnel `run_proposal`, shared prefilter and patch preparation, `apply_at` |
| `types.rs` | `Epoch`, `EvalPair`, `Evaluator`, `ApprovalGate`, `ApplyAt`, `CandidateOutcome` (`Proposal`, `Prediction`, `EvalStage`, `CandidateState` and `ReasonCode` live in `crates/rsi-ralph-types`) |
| `normalized/` | Normalized-H funnel `run_proposal_h` with `HEpoch`, `HEvaluator`, `HApprovalGate`; see [its README](normalized/README.md) |
| `test.rs`, `test_funnel.rs` | Rejection reasons, promotion, deduplication |

## Two funnels

- **`run_proposal_h`** (normalized) is the one the epoch driver and the live
  examples use. Predictions must use `metric = "normalized_h"`, stages are
  scored with `compare_h` and the human sees a `ScoreComparison`.
- **`run_proposal`** (raw) scores stages with raw verified quality through
  `compare` and asks an `ApprovalGate`. It is kept for the legacy
  `live_openrouter` smoke run and is not an H adapter.

Both share the prefilter, patch preparation, memory pinning and archive
events described below.

## Funnel order

`run_proposal` processes one class-scoped proposal through grounded prediction,
exact deduplication, language guard, isolated memory pinning, screening, full
selection, independent confirmation and an explicit human gate.

| Step | Stops with |
| --- | --- |
| Patch class differs from `Epoch::class` | `WrongClass` |
| Empty fields, wrong metric, non-positive gain, no nodes | `Untestable` |
| Transcript refs or nodes not in the epoch | `Ungrounded` |
| Same patch already recorded or proposed | `Duplicate` |
| Patch shape cooled by recent strikes (`Archive::penalties`, `Epoch::penalty`) | `Cooldown` |
| Declaration written to the archive (both funnels) | — |
| Patch fails to apply (`patch::apply`) | `Unformalizable` or `Guard` |
| Read-only memory, goal acceptance or untouched watermark changed | `Guard` |
| `Evaluator::pin_memory` fails | `HostFailure` |
| Candidate content ID already in the archive | `Duplicate` |
| Prepared candidate written to the archive | — |
| Screen: reject | `NoGain` |
| Selection: reject or inconclusive | `NoGain` / `Inconclusive` |
| Confirmation reuses a task or seed, rejects, or is inconclusive | `NotReplicated` |
| Evaluator or approval service error | `HostFailure` |
| Evaluator reports `Error::CandidateFailed` (candidate failed runs the incumbent completed) | `Harmful`, after a `CandidateFailureEvent` |
| Human says no | `ApprovalDenied` |
| Human says yes | `Accepted`, with `apply_at` set |

A screen result of inconclusive continues to selection; only reject stops.

## Public surface

- `run_proposal(archive, &Epoch, &Proposal, evaluator, approval) -> CandidateOutcome`
- `run_proposal_h(archive, &HEpoch, &Proposal, evaluator, approval) -> HCandidateOutcome`
- `apply_at(PatchClass) -> ApplyAt`: `Context`/`Memory` → `NextTurn`,
  `Prompt` → `ReplaceAgent`, `Roster` → `ManageRoster`,
  `Topology`/`Policy`/`Goal` → `BetweenSessions`.
- Host ports: `Evaluator` (`pin_memory`, `evaluate`), `HEvaluator`
  (`evaluate_h`), `ApprovalGate` (`approve`), `HApprovalGate` (`approve_h`).
  `CoordinatorEvaluator` in `crates/rsi-ralph-host/src/suites/` implements both evaluator traits.

Both outcomes carry the `ReasonCode`, the candidate ID when one was formed,
the last comparison, and `apply_at` only for approved designs.

## Operational constraints

The archive head is a selected design pointer, not proof that a live process
has applied it. The host executes the returned boundary and restores the
parent package/memory on revert. Evaluators enforce hard caps during execution
and never share mutable memory across arms. Every proposal needs a checkable
prediction and real public evidence. Raw ±2SE comparison is a heuristic;
scientific runs need normalized bootstrap scoring and frozen anchor suites.

Today no host code reads `apply_at`. `CoordinatorHost` performs a full
rebuild for every class, so the finer boundaries are advisory until a host
implements them.

Failed host attempts are also deduplicated. A prepared snapshot is retained for
audit, not treated as a resumable evaluation checkpoint. Retrying private
evaluation requires a separately specified recovery protocol that pins the
incumbent, epoch, suites, seeds, budget and policy, and distinguishes unobserved
infrastructure failures from partially observed trials. Reusing package and
watermarks alone does not establish those invariants. This API therefore does
not automatically retry `HostFailure` attempts; host recovery is part of P2.

Approval gates in the examples are scripted (`Deny`, `ContextGate` in
`examples/complete/fixtures.rs`). A production host must supply a gate backed
by a real human decision on the exact candidate ID.
