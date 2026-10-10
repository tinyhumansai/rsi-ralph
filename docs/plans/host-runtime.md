# Host runtime implementation and verification map

This document maps the implemented [host contract](../specs/host-runtime.md)
to source and regression coverage. It replaces the earlier foundation-only
acceptance list; the final live experiment report remains separate evidence.
No vendor modifications are required.

## 1. Preserve selected design and measured evidence

Source: `crates/tinyrsi/src/archive/{mod,types,replay,normalized}.rs` and
`crates/tinyrsi/src/orchestrator/normalized/`.

Implemented sequence: validate proposal/guards, write normalized preregistration,
pin and prepare the candidate, persist Screen/Selection/Confirmation runs,
record the exact human decision, and conclude record/outcome atomically.
Replay verifies chronology, frozen policy/suite identities, recomputed comparisons
and accepted confirmation/approval. Archive tests cover direct append bypasses,
policy replacement, duplicate candidates, corruption and transactional failures.

## 2. Provide immutable local memory

Source: `crates/tinyrsi/src/memory/mod.rs` and `memory/test.rs`.

`SnapshotMemoryStore` pins owned namespace/watermark maps without changing active
memory. It validates entry preconditions, writable identities, forks, canaries and
size limits. Active selection restores exact historical images independently.
Tests cover stale entries, immutable shadow learning, restart, retired identities,
forked images and exact reversal. It implements the `MemorySnapshot` host port;
other engines can implement the same semantics. This is not a CortexDB adapter.

## 3. Bind actual verification to coordinator completion

Source: `crates/tinyrsi/src/verification/{mod,types,runner}.rs`,
`verification/test.rs` and `verification/test/runner.rs`.

Immutable `VerifierRegistry` checks references, schema, direct executables and
restricted Lean proofs. `VerifiedRunner` owns episode-local DAG state, rejects
forged artifacts and calls completion only after required host checks pass.
Tests cover failed/forged parking, actual coordinator settlement, multi-seat DAG
routing/release, subprocess bounds and proof admission/kernel axiom rejection.
The opt-in `live_stock_lean_checks_frozen_theorem_and_kernel_axioms` requires
`TINYRSI_LEAN`; the complete live example also requires actual stock Lean.
Hosts supply sandboxing and immutable process checker workspaces.

## 4. Evaluate private draws and install a concrete runner

Source: `crates/tinyrsi/src/suites/{types,fingerprint,runner,mod,factory}.rs`.
Tests: `suites/test.rs`, `suites/test_private.rs`.

`PrivateSuites::freeze` fingerprints four disjoint stages without secret Debug
or serde projections. `CoordinatorEvaluator` uses fresh storage per arm/task/seed,
owned memory images, real host checks and external token metering. One-arm anchor
subdraws retain the frozen protocol identity. Tests cover exact pairing, failed
verification credit, memory isolation, checker mutation, model errors, forged
usage, unsupported settings and nested runtime rejection.

`ControlledRunnerFactory` defers its verified runner until `attach` supplies the
new writer. It supports one artifact-only seat, validates settings explicitly,
preserves logical sessions and meters deployed outcomes. Missing writer binding
parks without a model call. Generic `RunnerFactory` implementations own broader
capabilities. Deployment/evaluation use the same conservative scheduler cap map.

## 5. Make activation and observation durable

Source: `crates/tinyrsi/src/host/{mod,types,runtime}.rs`.
Tests: `host/test.rs`, `host/test_runtime.rs`, `host/test_store.rs`.

`DeploymentStore` records selected/deployed separation and intent before mutation.
`CoordinatorHost` preflights configuration/memory, fences old writers and rebuilds
with retained sessions and exact membership. Failed activation compensates to
old package/memory through another fresh coordinator. Parent rollback reconciles
archive and deployment after interruption. Ordinary restart reattaches queued
work under the unchanged design. Activation remains quiescence/admission gated.

Regressions cover SQLite reopen, writer exclusion/fencing, pending intent failure,
post-fencing registration/attachment compensation, missing snapshot protection,
roster order/dormancy, parked and queued blockers, exact restored memory, durable
observation cursors and terminal evidence consumed once. Snapshot write failures
preserve deployment and do not consume observations.

## 6. Run durable Engine epochs and retain all attempts

Source: `crates/tinyrsi/src/epoch_driver/{generation,journal,mod,types}.rs`,
`retreat_memory/` and `model/`.

`prepare_retreat_epoch` validates persisted observations, freezes normalized
requests/instructions/seeds, saves intent, then runs actual tinybackrooms or the
direct proposer. Every strictly decoded formatter occurrence is retained;
invalid outputs remain counters. `DurableRetreatMemory` supplies scoped discovery
and labelled outcome recall. `evaluate_retreat_epoch` invokes normalized selection
and records completion. Reopen never repeats completed provider calls or approval;
unknown interrupted effects fail closed. Tests exercise actual Engine failure/
repair accounting, durable recall, all-attempt capture and restart reuse.

`OpenRouterModel` supplies the concrete provider port. `BudgetedModel` reserves
conservative input/output caps before dispatch, charging failures and retries.
Credentials remain inherited host configuration.

## 7. Separate experiment machinery from efficacy

Source: `crates/tinyrsi/src/experiment/`; contracts:
[ignition specification](../specs/ignition.md) and [implementation map](ignition.md).

`ExperimentProtocol` freezes all arms/draws/caps before output; `run_experiment`
checks exact provenance, complete attempt ledgers, paired anchors and independent
trial bootstrap. Full, MemoryOff and RetreatOff are explicit interventions.
Missing/invalid evidence prevents positive claims; scripted runs are ProtocolOnly.

`crates/tinyrsi/examples/complete_openrouter.rs` with `examples/complete/`
executes the worked host lifecycle followed by 24 bounded live assignments.
Final measured results are pending the report. A negative/inconclusive result
must be retained, and no result from this bounded harness is called ignition.

## Validation commands

Run from the workspace root:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
.github/scripts/check-file-coverage.sh 90 coverage.json
```

The parent integration task owns the final command results and live report;
this map describes implemented behavior, not an unrun verification claim.
