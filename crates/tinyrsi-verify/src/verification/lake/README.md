# Pinned Lake checker

`HostVerifier::Lake` stages a fresh host-owned `lakefile.toml`, exact
`lean-toolchain` and frozen `Rsi.lean` under `CommandSpec.workspace_root`.
The explicit default library target prevents an empty successful build from
being mistaken for a proof. Only the restricted `by` proof block is generated.

Configure an absolute path to the installed toolchain's actual `bin/lake`;
Elan dispatch shims are rejected. Its neighboring `bin/lean --version` must
match the stable declared version. Version probing and building share the
configured total time/output limits. A successful build must emit exactly one
source-bound kernel report with only the stock axiom allowlist. `sorry`, invalid
proofs and native execution axioms cannot receive credit.

Suite identities bind both executable hashes, the project template, frozen
theorem, exact version and limits. The host must keep the entire installed
toolchain immutable, including core libraries and dynamic dependencies; those
files are not individually hashed. Frozen preludes are trusted host inputs.
This project has no external dependency declarations and does not fetch them.
The demonstrated slice imports only installed `Std`.

This is a restricted proof adapter, not an OS sandbox for arbitrary Lean
metaprograms. Existing process-group cleanup and cleared environments apply.
Use checkout `target/` for staged sources and build products. Private sources,
proofs and diagnostics never enter public verification evidence.
