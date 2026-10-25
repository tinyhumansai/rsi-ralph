# The RSI Ralph library

The `rsi-ralph` binary covers loops defined as folders. Hosts that need more
control link the Rust crates directly. `crates/rsi-ralph` is the library a host
depends on: it has no daemon and no plugin surface of its own. This page is
the tour of what the crates do; the API reference is the rustdoc
(`cargo doc --no-deps --all-features --open`).

## The host-driven hive loop

The Rust API runs a host-controlled improvement loop: persist redacted epoch
observations, generate typed proposals, freeze normalized evaluation policy and
private suite identities, evaluate in isolated coordinator episodes, and ask a
human to approve the exact confirmed candidate. `Archive` retains original
language `Record` and `Package` values, prepared snapshots, declarations, paired
runs, recomputed comparisons, explicit decisions and final outcomes. Selection
and live deployment remain separate durable facts.

The `rsi-ralph-host` crate supplies `CoordinatorHost` over real tinyhivemind
storage, including SQLite, and `DeploymentStore` for activation, compensation,
restart and parent rollback. `SnapshotMemoryStore` provides immutable local
memory images. `VerifiedRunner` settles work only through immutable host checks;
`CoordinatorEvaluator` runs frozen private suites with fresh coordinator storage
per task, seed and arm. `EpochJournal` preserves real retreat attempts and
prevents completed generation/evaluation from repeating after restart.

`EvidenceJournal` persists controlled turn intent, provider-measured usage and
observed checker evidence, then atomically consumes them into a redacted epoch
receipt. Duplicate callbacks and repeated epoch reads do not recount work after
restart; interrupted calls with missing usage remain explicitly unknown. Run
`cargo run -p rsi-ralph-host --example observe_epochs` for the offline
two-epoch demonstration and public receipts under `target/rsi-observe-epochs/`.

`EpochJournal::proposal_dataset` links typed formatter occurrences and committed
predictions to selection-only labels, preserving missing and censored outcomes.
Optional probabilities are frozen before evaluation; Brier, tied-rank Spearman
and precision-at-k report their denominators without using confirmation or anchor
measurements. See the [dataset contract](../crates/rsi-ralph-host/src/epoch_driver/dataset/README.md)
and the `proposal_metrics` example for reporting an existing private host run.

The supplied `ControlledRunnerFactory` is a bounded, artifact-only, single-seat
adapter. It installs supported prompts, context, recall and conduct caps, and
rejects unsupported models, tools, MCP and policies explicitly. A custom
`RunnerFactory` owns broader deployment capabilities. Multi-seat verified DAG
execution is exercised separately. These adapters use existing upstream APIs;
this repository does not modify OpenHuman or provide a CortexDB memory adapter.

`ExperimentProtocol` and `run_experiment` preregister original/evolved arms,
private draws, equal total caps, ablations and an aggregate reporting rule.
The [complete live run](rsi/complete-live.md) passed separate acceptance and
SQLite deployment/rollback validation; its 24-attempt comparison was inconclusive.
These public toy fixtures do not establish positive RSI or ignition.
See [phase status](rsi/phases.md), [host runtime](specs/host-runtime.md)
and [the experiment protocol](specs/ignition.md).

## Improving a plain script

`run_script_loop` repeats observe, ideate, execute, measure, score and improve
over any script that prints `METRIC <name>=<value>` (plus optional
`DIAG <text>` lines the supervisor reads), supervised by a language
model, and streams every stage to `events.jsonl`. Try it offline on a
curve-fitting script with `cargo run -p rsi-ralph --example script_loop`; point
it at your own script with the [script loop guide](guides/script-loop.md).
Proposed code runs locally without a sandbox; read the guide's safety note
before live runs.

## End-to-end examples

[`examples/`](../examples/README.md) runs the whole loop (evolve, score, keep
what is better) on four problems, offline by default:

- `math_problem`: compute π to 12 digits.
- `upgrade_file`: bring a file up to a written coding standard.
- `graph_coloring`: colour random graphs with as few colours as possible.
- `hive_goal`: improve a hive of agents until they solve private tasks.

Run `cargo run -p rsi-ralph-examples --example math_problem` (or
`upgrade_file`, `graph_coloring`, `hive_goal`), then watch the runs in `apps/rsi-viz` with
`RSI_RUN_ROOT=../../target/rsi-examples`.

## Live runs

The live examples spend real API tokens and do nothing unless
`TINYRSI_LIVE_OPENROUTER=1` is set. How to run them, and what each one proves,
is in [development.md](development.md#live-runs).
