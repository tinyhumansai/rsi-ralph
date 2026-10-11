# Private benchmark implementation

Contract: [host-owned inputs](../specs/private-euler-benchmarks.md).

1. Test independent assignment, Harshad-prime, idempotent and Euclidean-step
   oracle pairs against small known examples, boundaries and randomized inputs.
2. Implement standard-library-only independent algorithms and private bank
   generation. Test unique stage identities/seeds, reproducibility from private
   seed, immutable output and public evidence redaction.
3. Check official problem pages and immutable published solution source;
   retain hashes, difficulty and published-answer provenance in a public catalog.
4. Add a Rust example that binds each private row to a golden checker and freezes
   all four draws. Test actual loading, malformed banks and numeric answer format. Generate
   correct/off-by-one private calibration probes and observe every label through
   the actual immutable golden registry.
5. Generate a fresh private bank, load it with the Rust adapter and publish only
   source evidence, counts and opaque fingerprints. Run Python tests and the four
   Rust contract commands before opening a ready upstream PR.
