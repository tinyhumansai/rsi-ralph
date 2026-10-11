# Documentation

This directory holds documentation that does not belong in rustdoc: the shape
of the system, the reasoning behind it, and the constraints a reader needs
before touching the code. API reference lives in doc comments next to the code,
where it cannot drift.

## Start here

1. [`architecture.md`](architecture.md) — one epoch of the improvement loop,
   which function owns each step, and what is still missing.
2. [`glossary.md`](glossary.md) — epoch, proposal, candidate, incumbent,
   stage, dossier, H score, anchor, ApplyAt and the rest, each tied to a type.
3. [`rsi/loop-walkthrough.md`](rsi/loop-walkthrough.md) — the same epoch
   traced through the live example and every file it writes.
4. [`rsi/README.md`](rsi/README.md) — research index: phase status, remaining
   work, scoring, experiments and measured results.

The repository root has [`ROADMAP.md`](../ROADMAP.md) (shipped, next, out of
scope) and [`MODULE.md`](../MODULE.md) (what the released TinyBus module
serves).

## Where docs live

Documents are grouped by kind, not by subsystem. Nothing has been moved to
make the layout tidier, because many files link to each other by path.

```text
docs/
├── README.md         # this index
├── architecture.md   # system overview
├── glossary.md       # term definitions
├── specs/            # contracts for code in this repository
├── plans/            # implementation sequences for those specs
├── adr/              # architecture decision records, numbered and immutable
└── rsi/              # the RSI research track
    ├── specs/        # research-level framework and scoring specs
    ├── results/      # dated, published run outputs (JSON)
    └── *.md          # status pages, audits, and dated run reports
```

- **[`specs/`](specs/README.md)** — one file per feature or subsystem of this
  crate (host runtime, epoch evidence, generation receipts, proposal
  outcomes, proposal dispositions, pinned Lake, ignition experiment, private
  benchmarks, module release). Each states behavior, invariants and
  acceptance criteria that the code must meet.
- **[`plans/`](plans/README.md)** — the test-first steps that delivered a
  spec, with the same file stem (`specs/host-runtime.md` ↔
  `plans/host-runtime.md`). Plans are records of how work landed; the spec
  stays the source of truth.
- **[`rsi/specs/`](rsi/specs/)** — the research framework the crate
  implements: [`rsi-framework.md`](rsi/specs/rsi-framework.md),
  [`orchestrator.md`](rsi/specs/orchestrator.md),
  [`scoring.md`](rsi/specs/scoring.md) and
  [`way-of-working.md`](rsi/specs/way-of-working.md). They describe the loop
  conceptually and are broader than any one module.
- **Dated run reports** — pages under `rsi/` that describe a specific run,
  such as [`rsi/complete-live.md`](rsi/complete-live.md),
  [`rsi/live-smoke.md`](rsi/live-smoke.md),
  [`rsi/p1-evidence.md`](rsi/p1-evidence.md) and
  [`rsi/pinned-lean.md`](rsi/pinned-lean.md). Their raw outputs are in
  `rsi/results/<date>/`. These pages are historical evidence: correct a
  factual error, but do not rewrite them to match later code.
- **[`adr/`](adr/0001-record-architecture-decisions.md)** — one dated record
  per significant decision. An accepted ADR is not edited; it is superseded
  by a later one.

Module-level design notes live next to the code in
`crates/tinyrsi/src/<module>/README.md`.

## Conventions

- Keep every Markdown file at 500 lines or fewer. When a topic outgrows that,
  split it into focused files and link them from the nearest `README.md`.
- Update documentation in the same commit as the behavior it describes.
- Prefer a concrete example over an abstract description.
- Link between documents rather than duplicating content; one fact lives in one
  place.
- Write a specification before a plan: the spec defines the outcome and
  constraints, while the plan defines the implementation sequence.
