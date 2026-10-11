# Pinned Lake verifier implementation

Contract: [private project checks](../specs/pinned-lake.md).

1. Add failing tests for the new verifier, frozen project files, version mismatch,
   absent/duplicate/kernel-forbidden reports and proof-input rejection.
2. Extend host verifier types and proof-tier classification. Factor bounded
   process staging to accept host-only files; retain direct Lean compatibility.
3. Validate actual neighboring binaries, stable version and exact build command.
   Fingerprint both binaries and the immutable project template.
4. Parse Lake source diagnostics, share total time/output limits and test process
   failures without weakening completion or evidence rules.
5. Run the genuine pinned toolchain on a private finite mathematics slice.
   Publish safe identities, counts, fingerprints and observed exits only.
   Run four Rust checks, strict rustdoc and coverage gates.
