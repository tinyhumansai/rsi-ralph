# Verified decomposition

Status: Accepted contract; runtime integration is partial.

Related: [P3 issue #5](https://github.com/tinyhumansai/rsi-ralph/issues/5),
[phase status](../phases.md), and the
[implementation plan](../../plans/verified-decomposition.md).

## Problem

A hive can split a task into dependent claims, but its own claims cannot be
accepted as evidence that those claims passed. The host needs frozen grading
checkpoints, real checker observations tied to the exact work checked, and one
completion gate shared by hive and single-agent evaluations. Restarting or
replaying records must not turn a claim or a prior status into a new checker
pass.

## Goals and non-goals

- Bind every runtime node to one immutable host verifier and authorized seat.
- Bind each report to episode, seat, node, artifact digest, observed command
  outcome and visible transcript sequence references.
- Keep mutable node status in host runtime state, separate from editor-owned
  package data. Failed prerequisites revoke dependent passes.
- Admit completion only after every frozen required check has an observed pass.
- Use proof, execution, schema and held-out references ahead of rubric and
  preference judgments; mark the latter as weak evidence.
- Keep eval-owned checkpoints and per-node credit independent of hive-created
  decomposition.
- Exercise a pinned code suite in an isolated workspace and a private Lean
  slice with exact toolchain and task identities.
- Keep checker configuration and acceptance criteria unchanged under every
  allowed patch class.

This contract does not implement a generic OS sandbox, change tinyhivemind's
completion API, make model-authored proof tasks independent benchmark evidence,
or allow runtime status to substitute for checker execution. Hosts remain
responsible for configuring a sandbox suitable for commands that execute
generated code. The generic completion gate remains owned by
[tinyhivemind #100](https://github.com/tinyhumansai/tinyhivemind/issues/100).

## Behavior

### Frozen checkpoints and runtime DAG

The host freezes the required node identities, statements, verifier identities,
authorized seats, dependency edges and acceptance criteria before candidate
patches are applied. Runtime decomposition may schedule work against these
checkpoints, but cannot add, remove or rewrite a grading checkpoint. A package
may carry proposed work, never verified status or verifier evidence.

The host rejects empty or duplicate identities, unknown or repeated dependency
edges, cycles, missing verifier declarations and declarations whose node or
authorized seat does not match the frozen graph. A node is ready only after all
its prerequisites have observed passes. Failure or invalidation of a prerequisite
returns verified descendants to an unverified state.

### Checker execution and evidence

Only a host-owned registry may select the checker. The registry freezes a
digest of the checker configuration and executable bytes; changed command
settings or executable bytes reject verification. The host stages the exact
artifact in its configured workspace and executes the declared check directly,
with frozen arguments, time and output bounds, and a cleared child environment.
Shell interpolation is never used. Executing generated code requires a
host-provided sandbox wrapper; the process runner alone is not a sandbox.

An accepted report records the episode, authorized seat, frozen node, exact
artifact digest, SHA-256 checker identity over frozen configuration and
executable bytes, observed exit status, plus
the transcript sequence references visible to that seat. Evidence excludes
private expected values, proof source and raw checker output. A seat's claim,
forged binding, missing checker run, nonzero exit, timeout, output-limit failure,
or failed evidence persistence cannot produce a pass.

Checker evidence may be durably retained across restart. Reopening or replaying
that evidence does not itself mark a node passed: the host must validate the
record against its frozen declaration and active episode, and completion still
requires a successful observed checker outcome for every required node.

### Evidence strength and scoring

Prefer proof checked by the stock kernel, executed tests, schemas and held-out
references in that order of applicability. Rubric and preference judgments are
weak evidence and must be flagged in reports. Open, sorry, failed and invalidated
nodes remain explicit. Reports attribute per-node credit to eval-owned
checkpoints and include verified nodes per token. Self-created decomposition
cannot change the denominator or create scoreable checkpoints.

### Shared completion gate

Hive and single-agent comparisons use the same host verifier registry and
completion gate. The producer has no coordinator completion capability. The
host parks incomplete work and submits completion only after observed passes for
all frozen required checks. Checker or evidence-store failure retains whatever
safe measured evidence already exists but never bypasses the gate.

### Required checker fixtures

The pinned code suite runs its declared tests against a candidate in a fresh,
host-owned isolated workspace, with exact suite and command identities. The
private Lean slice runs `lake build` using an installed exact toolchain and
frozen task identities. Proof success requires the stock kernel evidence, not
an agent claim or a simulated Lean result. Public receipts may expose safe task
IDs, fingerprints, counts and exit dispositions, but not private tasks,
proofs, expected values or diagnostics.

## Invariants and constraints

- Verifier definitions and eval-owned acceptance criteria are immutable during
  candidate evaluation.
- Runtime status is host-owned and is never accepted from a candidate package.
- A report binds the checker result to the exact episode, node, seat, artifact
  and transcript observations.
- A report can pass only after the host actually runs the authorized checker
  and observes success.
- Invalidating a prerequisite invalidates every dependent pass.
- Failed persistence, restart or replay cannot bypass completion.
- Private checker inputs and output remain private.
- The host supplies any OS sandbox; no isolation guarantee is inferred from
  bounded process execution alone.
- Upstream typed goal-verifier declarations are integrated through their
  canonical immutable language record, not reimplemented in RSI Ralph.

## Acceptance criteria

1. Tests reject malformed graph identities, dependencies, cycles, mismatched
   declarations, forged reports and candidate-authored passes.
2. Evidence tests assert exact node and artifact binding, observed command,
   actual exit status and transcript sequence references.
3. Tests prove a failed prerequisite invalidates dependent passes and neither
   single-agent nor hive completion can settle before required observed passes.
4. Tests prove checker failure, evidence persistence failure, restart and replay
   retain available evidence without creating an unobserved pass.
5. A pinned code fixture runs tests in a fresh isolated workspace, and the
   private Lean fixture runs actual `lake build` with frozen task/toolchain
   identities and stock kernel evidence.
6. Reports expose weak-evidence flags, explicit open/sorry/failed nodes,
   eval-owned per-node credit and verified nodes per token; hive decomposition
   cannot alter scoring checkpoints.
7. Patches to every allowed package class leave verifier definitions and
   acceptance criteria unchanged.

## Current implementation and remaining work

`rsi-ralph-verify` currently implements the validated runtime DAG, immutable
verifier registry, real process/schema/golden/Lean/Lake checks, identity-bound
evidence and `VerifiedRunner`. `VerificationEvidence` includes a checker
identity digest. The durable journal rejects new callbacks without that digest
while retaining historical records written before the field existed.
`rsi-ralph-host` journals checker observations;
coordinator tests cover multi-seat dependencies and the completion gate. The
private Lean example runs 12 agent-authored finite tasks through genuine pinned
Lake builds. These are adapter checks, not independent model-performance
evidence.

Still outstanding are a pinned isolated code suite, a production sandbox
configuration for code execution, broader independently curated proof tasks,
and integration with the immutable typed goal-verifier declaration tracked in
[tinyhivemind #122](https://github.com/tinyhumansai/tinyhivemind/issues/122).
The generic upstream completion mechanism is not duplicated here.

## Open questions

- Which host deployment owns and attests the production code sandbox image and
  resource policy?
- Which independently curated proof tasks and reviewers will establish a
  private Lean slice beyond finite adapter certificates?
- What exact public typed declaration lands in tinyhivemind #122, and how will
  its stable identity be included in RSI Ralph suite fingerprints?
