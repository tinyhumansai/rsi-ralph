# Pinned private Lean project checks

Related: issues #1 and #5. Existing direct-Lean proof checks do not satisfy
the private project slice acceptance criterion.

Add a proof-tier verifier constructing a fresh host-owned Lake project under
the configured checkout workspace root. The immutable project has one `Rsi`
library, an explicit default build target, a frozen `rsi_goal` theorem prelude
and an exact stable Lean toolchain declaration. Only a restricted proof block is
model-controlled. Execute the actual pinned Lake binary with `lake build`.

Bind suite fingerprints to both Lake and its neighboring Lean executable bytes,
the declared version, theorem, project template and execution limits. Reject
Elan shims, mismatched versions and changed checker definitions. The host supplies
an immutable installed toolchain; no dependency fetching or model edits to
project files are allowed. The project uses installed core/Std only.

A zero build exit is insufficient. Require exactly one stock kernel axiom report
for `rsi_goal` from frozen `Rsi.lean`, admitting only the existing standard axiom
allowlist. Reject missing/duplicate/forged reports, sorry, invalid proofs, native
execution axioms, timeout and excessive output. Retain generic identity-bound
evidence without private theorem, proof or process output.

Version checking and building share the total timeout and output allowance.
Process-group cleanup and fresh workspaces preserve checker isolation. This is
a restricted proof adapter, not a sandbox for arbitrary Lean metaprograms or
projects with mutable external dependencies.

Demonstrate an agent-authored private finite mathematics slice with exact task
identities and genuine builds. Do not label tasks human-curated or claim broad
formal theorem-proving performance. Upstream immutable typed goal declarations
remain tracked separately in tinyhivemind #122.
