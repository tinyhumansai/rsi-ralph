# Specifications

Specifications define what the system must do before implementation details
take over. Create one for behavior that changes a public API, crosses module
boundaries, introduces a durable data format, or has meaningful operational
constraints.

Use a short kebab-case filename such as `host-runtime.md`. Each specification
should contain:

1. **Status and owner** — Draft, Accepted, Implemented, or Superseded.
2. **Problem** — the user or system need, without prescribing a solution.
3. **Goals and non-goals** — the exact boundary of the work.
4. **Proposed behavior** — public API, inputs, outputs, errors, and examples.
5. **Invariants and constraints** — properties every implementation must keep.
6. **Acceptance criteria** — externally observable pass/fail conditions.
7. **Open questions** — unresolved decisions that block acceptance.

After the specification is accepted, create a linked implementation plan in
[`../plans/`](../plans/README.md). Keep code snippets small enough to clarify
the contract; production code still belongs under `src/`.

See [`host-runtime.md`](host-runtime.md) or
[`epoch-evidence.md`](epoch-evidence.md) for specifications from this repository.

## Index

- [`loop-trace.md`](loop-trace.md) — the append-only `events.jsonl` loop trace.
- [`script-loop.md`](script-loop.md) — the plain-script improvement loop.
- [`hypothesis-ledger.md`](hypothesis-ledger.md) — approach labels, the
  approaches-tried summary and per-slot directives for the script loop.
- [`rsi-cli.md`](rsi-cli.md) — the `rsi-ralph` binary and file-defined loops.
- [`upgrade-log.md`](upgrade-log.md) — why a loop's hive or config changed
  between runs: records, the run-start diff and `rsi-ralph upgrades`.
- [`score-history.md`](score-history.md) — every score of a loop across runs
  and epochs, scoring wall time, and `rsi-ralph history`.
- [`rsi-viz.md`](rsi-viz.md) — read-only live visualizer for loop traces
  and run snapshots (`apps/rsi-viz/`).
- [`crate-split.md`](crate-split.md) — score, evolve, core and observe
  crates; TinyBus module removal.
- [`hive-penalties.md`](hive-penalties.md) — attributing candidate failures,
  the strike ledger, cooldown, revocation and generator feedback.
- [`loop-benchmarks.md`](loop-benchmarks.md) — benchmark × model × repeat
  runner and comparison table for the script loop.
- [`../rsi/specs/verified-decomposition.md`](../rsi/specs/verified-decomposition.md)
  — frozen host checkpoints, observed verifier evidence and completion.
