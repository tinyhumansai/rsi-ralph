# Verified decomposition implementation plan

Contract: [verified decomposition](../rsi/specs/verified-decomposition.md).
Related: [P3 issue #5](https://github.com/tinyhumansai/tinyrsi/issues/5).

This plan closes the missing P3 integration around the existing verifier and
completion machinery. Do not duplicate tinyhivemind's generic completion gate
or replace the current Blueprint and VerifiedRunner APIs without a failing
acceptance test. The pinned Lake adapter and finite agent-authored Lean slice
already exist; extend them only where a task below requires it.

The command identity evidence requirement is implemented: `VerifierRegistry`
freezes checker configuration and executable bytes, and each new evidence
record carries that digest with its observed exit status. The tasks below cover
the remaining host and evaluation integrations.

## Assumptions and boundaries

- Work in `crates/tinyrsi-verify` and `crates/tinyrsi-host`; do not edit vendored
  tinyhivemind. Integrate its typed immutable declaration after upstream #122
  lands, and raise any required language addition on the upstream issue.
- Preserve current public evidence redaction and the existing coordinator
  completion API.
- Never describe finite agent-authored Lean tasks as curated or independent
  model-performance evidence.
- Production sandbox provisioning is a host deployment responsibility. TinyRSI
  can pin and test the configured wrapper identity, but cannot claim to provide
  the OS boundary itself.

## Tasks

### 1. Pin host verifier declarations to upstream typed goal identities

1. Add failing integration tests in
   `crates/tinyrsi-host/src/suites/test_private.rs` showing that two tasks with
   different immutable goal-verifier identities cannot share a suite
   fingerprint, and that candidate package patches cannot alter the declaration.
2. After tinyhivemind #122 lands, consume its typed record in
   `crates/tinyrsi-host/src/suites/` and bind the stable declaration identity
   into `fingerprint.rs`. Reject missing, changed or mismatched identities.
3. Add serialization and restart tests for the frozen declaration and ensure
   replay validation recomputes its identity.
4. Update the suite and language boundary documentation. If #122 is not landed,
   stop this task and report the dependency; do not create a duplicate local
   language record.

### 2. Add a pinned isolated code-suite fixture

1. Add failing tests under `crates/tinyrsi-host/src/suites/` proving each run
   receives a fresh workspace, only the declared candidate artifact is staged,
   and checker output or files from an earlier run cannot influence the result.
2. Implement the smallest host suite adapter using the existing
   `HostVerifier::Process` and `CommandSpec` contract. Freeze the test command,
   suite identity, workspace policy, timeout and output limit; reject a changed
   command or workspace policy during evaluation.
3. Exercise pass, test failure, timeout, output limit, and workspace isolation
   deterministically with local fixture executables. Verify reports retain the
   observed exit status and artifact/transcript binding while excluding output.
4. Document that the fixture proves workspace separation only. Code execution
   in production still requires an explicitly configured sandbox wrapper.

### 3. Bind and validate production sandbox configuration

1. Add failing host configuration tests showing that a code-execution suite
   cannot be activated without a configured sandbox identity and resource
   limits, and that changing either changes the frozen suite fingerprint.
2. Add a typed host configuration field at the existing suite/deployment
   boundary; pass it only to the process checker and never to model-controlled
   package data.
3. Test that the configured wrapper is invoked by direct argv, receives the
   staged workspace and limits, and that wrapper failure or missing evidence
   parks completion.
4. Document the operational contract and state clearly that actual isolation
   depends on the host's sandbox implementation and deployment policy.

### 4. Expand independent proof validation

1. Add failing identity tests in `crates/tinyrsi-host/src/suites/test_private.rs`
   proving each private Lean task binds its exact statement, checker, toolchain
   and stage, and that altered values invalidate the frozen suite.
2. Add a separately curated, held-out finite Lean slice and validate reference
   proofs with the genuine pinned Lake executable via the
   `crates/tinyrsi-host/examples/verify_lean_slice/` path. Keep its task source
   and expected evidence private.
3. Test wrong toolchain, changed theorem/checker identity, failed build and
   forbidden kernel axioms. Public receipts must contain only safe identities,
   counts, fingerprints and observed dispositions.
4. Report task provenance and reviewer curation separately from successful
   adapter execution; do not infer model ability from host reference builds.

### 5. Prove scoring checkpoints and restart safety end to end

1. Add failing coordinator integration tests under
   `crates/tinyrsi-host/src/suites/test.rs` for both hive and single-agent
   evaluation: candidate-authored nodes cannot change required checkpoints,
   denominator or per-node credit.
2. Add or extend `test_journal.rs` cases to persist a checker pass, reopen the
   journal/runtime, and prove replay alone cannot settle an incomplete episode.
   Also cover prerequisite failure, checker failure and evidence-store failure.
3. Implement only the minimal host-owned checkpoint/status and report plumbing
   required by those tests. Keep safe measured evidence across restart while
   requiring a fresh valid host checker observation before completion.
4. Assert explicit open, sorry, failed and invalidated node reporting, weak
   rubric/preference flags and verified nodes per token.

### 6. Verify and update documentation

1. Run focused verifier and host suite tests while iterating.
2. Run the repository contract commands: `cargo fmt -- --check`,
   `cargo clippy --all-targets --all-features -- -D warnings`,
   `cargo build --all-targets --all-features`, and `cargo test --all-features`.
   Run long commands in bounded slices or in the background as required by the
   workspace workflow.
3. Update `docs/rsi/phases.md`, `docs/rsi/remaining-work.md`, verifier/suite
   READMEs, and the private Lean example README with the implemented evidence
   and the remaining scientific and deployment limits.

## Completion checklist

- [ ] Upstream immutable typed goal declaration integrated after #122 lands.
- [ ] Pinned code suite executes tests in fresh isolated workspaces.
- [ ] Production sandbox identity and limits are frozen and exercised.
- [ ] Independently curated private Lean tasks pass genuine pinned Lake checks.
- [ ] Hive and single-agent comparisons share eval-owned checkpoints and gate.
- [ ] Restart, replay and checker failures cannot create completion credit.
- [ ] Public reports retain safe identity-bound evidence and score boundaries.
- [ ] Contract commands pass and documentation reflects measured limits.
