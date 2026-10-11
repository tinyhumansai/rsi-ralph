# Roadmap

What exists, what comes next, and what is deliberately out of scope. Phase
numbers (P1–P6) follow [umbrella issue #1](https://github.com/tinyhumansai/tinyrsi/issues/1).
Implementation detail per phase is in [`docs/rsi/phases.md`](docs/rsi/phases.md);
the acceptance gaps are audited in
[`docs/rsi/remaining-work.md`](docs/rsi/remaining-work.md). For how the pieces
fit together, start with [`docs/architecture.md`](docs/architecture.md).

"Shipped" below means the mechanism exists and is tested. It does not mean a
phase is accepted or that the loop has produced a measured improvement; the
published 24-attempt experiment is inconclusive.

## Shipped

**Improvement loop library** (paths under `crates/tinyrsi/src/`)

- P1 observe and record: `Observer`, `Dossier` (`observation/`),
  `DeploymentStore::observe` (`host/`), `EvidenceJournal`
  (`evidence_journal/`) and the replay-validated `Archive` (`archive/`),
  including chronological proposal dispositions with accepted, rejected and
  unresolved duplicate history (`archive/dispositions/`).
  Demonstrated offline by the `observe_epochs` example.
- P2 human-gated shadow selection: `run_proposal_h` with frozen screen,
  selection and confirmation stages (`orchestrator/`), normalized H scoring
  and bootstrap (`scoring/`), private suites and `CoordinatorEvaluator`
  (`suites/`), immutable memory snapshots (`memory/`).
- P3 verified decomposition: `Blueprint` (`blueprint/`), `VerifierRegistry`
  and `VerifiedRunner` with process, golden, schema and Lean checks
  (`verification/`), plus a pinned-toolchain `lake build` checker
  (`verification/lake/`) validated on 12 private reference proofs
  ([`docs/rsi/pinned-lean.md`](docs/rsi/pinned-lean.md)).
- P4 creative retreat: `prepare_retreat_epoch` / `evaluate_retreat_epoch`
  with a durable `EpochJournal`, generation receipts and outcome datasets
  (`epoch_driver/`), the tinybackrooms request contract (`retreat/`) and
  `DurableRetreatMemory` (`retreat_memory/`).
- P5 structural deployment: `DeploymentStore::activate`, `rollback` and
  `recover` over `CoordinatorHost` with SQLite fencing and compensation
  (`host/`).
- P6 bounded experiment: `ExperimentProtocol` and `run_experiment` with
  original/evolved arms and memory/retreat ablations (`experiment/`).
- Model boundary: `ModelPort`, `OpenRouterModel`, `BudgetedModel` and
  `ModelConversant` with pre-dispatch token reservation (`crates/tinyrsi-model`).
- A private 48-task numerical benchmark bank and pinned Euler controls
  ([`docs/rsi/private-benchmarks.md`](docs/rsi/private-benchmarks.md)).

## Next

Ordered roughly by how much they unblock. Each item links back to the audit.

- **Multi-epoch outer loop.** Nothing runs epochs back to back today; each
  example runs one epoch per directory. A planned script-subject loop will
  drive repeated epochs against a small scripted subject, with a live
  visualizer alongside it.
- **Act on `ApplyAt`.** Approved designs carry an activation boundary, but
  `CoordinatorHost` always rebuilds between sessions. Turn-boundary context
  and memory updates and live agent replacement are not implemented (P5).
- **Production approval.** Replace the scripted `Deny` / `ContextGate`
  fixtures with a real exact-candidate human approval port (P2).
- **Detailed host rejection taxonomy.** Exact-duplicate history exists;
  formalization, reach/contamination and constraint/canary subtypes do not
  (scoring #2).
- **Curated suites and empirical anchors.** Multi-family private tasks with
  measured family anchors, broader independent proof tasks, and code and
  harness-engineering coverage (P2, P3).
- **Calibrated proposal ranking.** Calibrated priors, blinded pairwise
  judgments and cost-aware successive halving (P4).
- **Sequential testing and anchor history.** Preregistered alpha spending,
  persistent anchor history tied to ancestor rollback, and tamper halting
  (scoring #2).
- **A learned orchestrator**, then an independent preregistered
  original-versus-learned comparison (P6).

## Out of scope

- A TinyBus or other plugin surface. The lineage module and its native
  release archives were removed
  ([ADR 0002](docs/adr/0002-split-crates-drop-bus.md)); TinyRSI is a library.

- Modifying vendored or upstream engines from this repository. OpenHuman,
  tinyhivemind language and coordinator, tinybackrooms and memory engines
  (including a CortexDB adapter) are changed in their own repositories.
- Anything that cannot be tested deterministically in CI. Live model runs
  stay opt-in examples gated by environment variables.
- Evolvable guardrails. Approval gates, verifier configuration, private
  suites, scoring policy, permissions and secret handling are frozen host
  surfaces, never patch targets.
- Claims of open-ended recursive self-improvement or ignition from the
  bounded experiment harness.
- Convenience wrappers that hide the crate's error taxonomy from callers.
