# Implementation plans

Plans turn an accepted specification into a reviewable sequence of small,
verifiable changes. They explain how to build the behavior; the linked
specification remains the source of truth for what the behavior must be.

Use the same kebab-case stem as the specification. A useful plan includes:

- a link to the accepted specification;
- the goal, non-goals, and assumptions relevant to implementation;
- ordered tasks with exact file paths;
- a failing test before each behavior change;
- the minimal implementation needed to pass that test;
- documentation and public-export updates;
- focused and full verification commands;
- a completion checklist updated as tasks land.

Prefer tasks that can be implemented and reviewed independently. Include short
code snippets when they remove ambiguity, but do not paste entire future files
into the plan.

See [`epoch-evidence.md`](epoch-evidence.md) for a test-first plan delivered in
this repository.

## Index

- [`loop-trace.md`](loop-trace.md) — loop trace types, clocks and sinks.
- [`script-loop.md`](script-loop.md) — script loop, example and hive trace.
- [`hypothesis-ledger.md`](hypothesis-ledger.md) — approach labels, ledger
  summary, slot directives and temperature.
- [`rsi-viz.md`](rsi-viz.md) — loop visualizer (`apps/rsi-viz/`).
- [`crate-split.md`](crate-split.md) — crate-by-crate extraction order.
- [`hive-penalties.md`](hive-penalties.md) — four stacked penalty changes.
- [`loop-benchmarks.md`](loop-benchmarks.md) — shared benchmarks, measurement,
  runner.
- [`verified-decomposition.md`](verified-decomposition.md) — checker, sandbox,
  typed declaration and restart integration for P3.
