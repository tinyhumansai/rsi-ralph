# Roadmap

What exists, what is next, and what is deliberately out of scope for the
TinyRSI module: the lineage ledger of a recursive self-improvement loop run by
OpenHuman agents. See `docs/rsi/` for the research and the working spec.

## Shipped

- module layout, crate-wide error type, and the public re-export surface
- the `lineage` feature area: a step-ordered ledger of graded candidates with
  `Append` and `Head` served over the bus, and the incumbent query
- lint configuration in `[lints]`, enforced identically locally and in CI
- CI: format, clippy, build, test, per-file coverage, rustdoc, MSRV, and
  supply-chain checks
- a manual release workflow that versions, tags, and creates a GitHub release
  with TinyBus runtime/module assets

## Next

- environment snapshot addressing: content-addressed config snapshots the
  `GradeEntry::snapshot` field can point at
- grading persistence: per-task scores and seed counts behind the aggregate
  grade
- a `Reject` archive query so the loop can read its own rejected proposals

## Out Of Scope

- anything that cannot be tested deterministically
- convenience wrappers that hide the crate's error taxonomy from callers
- evolvable guardrails: approval gates, permissions, and secret handling are
  frozen surfaces, not ledger entries
