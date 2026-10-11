# Private pinned Lean checker validation

The [pinned Lake specification](../specs/pinned-lake.md) extends host verification
with an immutable project checked by actual `lake build`, using exact installed
Lean 4.34.1 executables. The generated proof cannot edit the project, prelude,
toolchain or checker settings. A zero exit alone is insufficient: one stock
kernel axiom report for the frozen theorem is required.

The [retained safe receipt](results/2026-10-11/private-lean-checker-validation.json)
records 12 successful genuine builds across screen, selection, confirmation and
anchor stages. Each stage contains a Euclidean division certificate, modular
idempotent and modular inverse statement drawn from private random parameters.
Only predefined task IDs, suite fingerprints, toolchain identity and observed
exits are published. Inputs and seeds remain host-private under `target/`.

These are agent-authored finite mathematics inputs and host-reference proofs.
They validate the checker path; they are not model trials, human-curated labels,
or evidence of general mathematical improvement. They are also separate from
our [hard public Euler controls](private-benchmarks.md).

Live regression controls compile valid proofs and reject an invalid proof,
`sorry`, and kernel dependencies introduced by `native_decide`. CI installs the
same release with a pinned SHA-256 and runs the live controls during both tests
and per-file coverage. Protocol fixtures additionally reject wrong versions,
empty builds, missing/duplicate reports and malformed source locations.

P3 remains open: broader private proof tasks, production code sandbox setup and
upstream immutable typed goal-verifier declarations still need integration.
The host must freeze all installed core libraries and dependencies in addition
to the executable hashes currently included in suite fingerprints.
