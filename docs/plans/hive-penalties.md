# Hive penalties implementation

Contract: [penalizing bad hive runs caused by patches](../specs/hive-penalties.md).

Delivered as four stacked changes; each ends green on the four contract
commands and the per-file coverage gate. Each behavior starts with a test that
fails for the expected reason.

## 1. Documentation

- `docs/specs/hive-penalties.md`, this plan, both indexes.
- New rows in the evidence-to-action table of `docs/rsi/specs/scoring.md`.
- Glossary entries for Harmful, Cooldown, Strike and Patch shape.

## 2. Attribution

1. `crates/rsi-ralph-types/src/failure/`: `CandidateFailureKind` and
   `CandidateFailure`, with serde pins. `ReasonCode::Harmful` pinned as
   `HARMFUL` in `proposal/test.rs`.
2. `Error::CandidateFailed` in `crates/rsi-ralph-score/src/error/` and
   `crates/tinyrsi/src/error/`, with the `From` mapping and message tests.
3. Orchestrator tests first (`orchestrator/normalized/test.rs`,
   `orchestrator/test_funnel.rs`): a mock evaluator returning
   `CandidateFailed` must stop `Harmful` and leave a `CandidateFailureEvent`;
   `HostFailed` must still stop `HostFailure`. Then map it in both `measure`
   paths.
4. Archive: `CandidateFailureEvent`, `Event::CandidateFailure`,
   `record_candidate_failure`, `candidate_failures`, replay validation, and a
   disposition test where a repeated harmful patch is `DUP_REJECTED`.
5. Suites tests first (`suites/test_attribution.rs`): a model that fails only
   for the candidate configuration yields `CandidateFailed`; one that fails for
   the incumbent yields `HostFailed`; an unsupported candidate hive and an
   unreservable candidate request are attributed without model calls. Then tag
   jobs by arm in `suites/mod.rs`, run the incumbent first, and classify
   producer failures in `suites/runner.rs`.
6. `epoch_driver/trace.rs`: `measured` carries the failure counts; test in
   `epoch_driver/test/evaluation.rs`.

## 3. Ledger, cooldown and revocation

1. `crates/tinyrsi/src/archive/penalty/`: `PatchShape`, `Strike`,
   `StrikeKind`, `PenaltyReport`, `PenaltyPolicy`; tests for shape equality,
   each strike source, exclusions and window ageing.
2. `ReasonCode::Cooldown` (`COOLDOWN`), non-terminal in dispositions.
3. `Epoch::penalty`; prefilter check after exact duplicates. Test first: a
   same-shape patch is cooled with zero evaluator calls (counting
   `HEvaluator`), and becomes eligible after the window.
4. `Event::Revocation` and `Archive::revoke`; `DeploymentStore::rollback`
   calls it. Test first: rollback records a `Revoked` strike and the record
   stays `Accepted`, across reopen.
5. Policy fingerprint always includes the penalty policy, default or not.
6. `RevocationReason` on the revocation event, pinned; `Archive::revocations`.
   `DeploymentStore::enforce_anchor` rolls back on an `AnchorSignal`
   regression with reason `anchor_regression`. Test first: regression rolls
   back, strikes and cools the shape; a widening gap alone does nothing;
   `NoParent` and pending errors propagate.

## 4. Generator feedback

1. `rsi-ralph-evolve`: `PriorIdea`, `PriorOutcome`,
   `build_retreat_request_with_priors`. Test first: seeds carry `outcome` and
   no private fields; the existing builder keeps its seeds otherwise.
2. `RetreatOutcomeStatus::{Harmful, Regressed}` and optional `shape`, pinned;
   recall folds an avoid line. Test first: round-trip and recall.
3. `epoch_driver`: strikes-first priors in `prepare_retreat_epoch`; `remember`
   records the new statuses.
4. `examples/hive_goal`: a crashing context edit rejected `HARMFUL` and its
   near-duplicate `COOLDOWN` in epoch 1.

## Verification

- `cargo +1.99 fmt --all -- --check`, `clippy --all-targets --all-features -- -D
  warnings`, `build --all-targets --all-features`, `test --all-features`.
- `.github/scripts/check-file-coverage.sh`.
- `cargo run -p rsi-ralph-examples --example hive_goal`; `npm test` in
  `apps/rsi-viz`.
