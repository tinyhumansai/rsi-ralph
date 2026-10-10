# Implement and exercise the frozen experiment

Specification: [bounded evidence](../specs/ignition.md).

1. Add typed immutable experiment registration and SHA-256 fingerprinting.
   Pin tests for invalid configuration IDs, suite reuse, overlapping trial
   seeds, insufficient attempts, zero/overflowing caps, and policy validation.
2. Execute the real host callback under both arms and three ablations.
   Test exact assignment matching, counterbalanced order, complete attempt
   ledgers, and counting rejected/failed calls and both anchor matrices.
3. Recompute paired normalized anchor H using the existing scoring engine.
   Test malformed matrices, frozen task shape, unequal initial outcomes,
   inconsistent health ratios, and aggregate/per-run cap violations.
4. Bootstrap independent paired trial efficiency using deterministic draws.
   Test known constant means, genuinely variable interval width, repeatability,
   negative evidence, missing-arm failures, and hard-gate failures. A sampler
   that repeats the same balanced draw must fail the variable-width regression.
5. Produce the redacted serializable aggregate report. Test JSON round trips
   and absence of private task contents. Scripted callbacks must produce
   ProtocolOnly even when their artificial gain is large.
6. Wire host private suites, capped model port, coordinator evaluator, normalized
   runner, archive isolation, and actual memory/retreat ablations in the final
   runnable example. Freeze the protocol before any model calls. A bounded live
   run may conclude Inconclusive; publish actual measured provenance and counts.
7. Run module tests, per-file coverage, and the repository's fmt, clippy, build,
   and test contract. Report measured live evidence without asserting ignition.
