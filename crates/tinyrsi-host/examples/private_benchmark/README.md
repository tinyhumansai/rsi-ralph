# Private numerical benchmark adapter

`private_benchmark <bank.json>` reads a host-controlled bank, rejects malformed,
overlapping or unbalanced draws and freezes four actual `PrivateSuites` stages.
Every task uses an exact-byte `HostVerifier::Golden` with a canonical integer and
newline. The adapter prints opaque suite fingerprints and the count of verified
automatic calibration samples only. Its internal decoded
types deliberately have no debug or serialization output.

The bank generator independently computes each answer twice. This adapter checks
configuration and freezes the supplied answers; it does not recompute them or
attest the honesty of a modified host configuration. Retain the bank hash and
generator revision, restrict access and freeze before any evaluated model calls.
The same bank can be loaded into `CoordinatorEvaluator`; a solver sees its task
prompt, while the proposer and public reports must never receive the bank.

These checkers verify exact numerical answers. They do not certify source code,
formal proofs, non-memorization or a measured normalized family anchor.

See [curation instructions](../../../../docs/rsi/private-benchmarks.md).
