# Script loop

**Status:** Accepted. **Owner:** TinyRSI maintainers.

Related: [loop trace](loop-trace.md), [guide](../guides/script-loop.md),
[hypothesis ledger and slot directives](hypothesis-ledger.md).

## Problem

TinyRSI improves tinyhivemind packages one epoch at a time and has no outer
loop. Many improvement targets are not hives at all: a plain script with a
measurable output (a curve fit, a solver, a benchmark) could be improved by the
same observe, ideate, execute, measure, score, improve cycle, supervised by a
language model, if the library could run that cycle end to end.

## Goals and non-goals

- Goal: a complete, repeating loop over a plain script: baseline, then per
  epoch observe, ideate K proposals, execute and measure each on fixed seeds,
  score each against the incumbent, promote the best accepted one, and stop on
  a declared reason.
- Goal: a durable ledger of every candidate and a live [loop trace](loop-trace.md).
- Goal: reuse the existing paired evaluation (`evaluation::compare`), model
  boundary (`ModelPort`) and process runner instead of parallel copies.
- Non-goal: refactoring the hive orchestrator onto a generic subject.
- Non-goal: sandboxing. See [Safety](#safety).

## Proposed behavior

`run_script_loop(subject, supervisor, policy, run, trace) -> Result<LoopReport>`.

- **Subject** (`ScriptSubject`): a workspace directory, the relative paths of
  the files the supervisor may rewrite (`editable`), the command to run
  (absolute or `PATH`-resolvable program plus arguments, run in the candidate
  directory), a `MetricSpec { name, direction, target }`, at least two distinct
  seeds, a per-seed timeout, an output cap, and optional extra environment.
- **Policy** (`LoopPolicy`): `max_epochs`, `proposals_per_epoch`, `patience`
  (epochs without promotion before stopping), `token_budget` (measured
  supervisor tokens, plus scorer tokens under `run_script_loop_with`), `rejected_context` (how many rejected proposals the
  supervisor sees), `approach_context` (how many tried approaches the
  supervisor sees), `slots` (one `SlotDirective` per slot, cycled), and the
  `EvalPolicy` used for scoring (margin zero by default).
- **Execute:** each candidate is materialized into
  `<run>/candidates/<content-id>/` by copying the workspace (symlinks and the
  run directory itself are skipped) and writing every editable file with the
  candidate's content. The content id is the first 12 hex characters of the
  SHA-256 over the sorted editable paths and contents. The command runs once
  per seed with a cleared environment containing only `PATH=/usr/bin:/bin`,
  `TMPDIR` (a fresh directory under `<run>/tmp/`), `TINYRSI_SEED`, and the
  subject's explicit extra variables; the working directory is the candidate
  directory; the run is killed at the timeout or when stdout plus stderr exceed
  the output cap.
- **Measure:** the last stdout line of the form `METRIC <name>=<float>` is the
  seed's metric. A seed fails on timeout (`ScriptTimeout`), output overflow
  (`ScriptOutputExceeded`), a non-zero exit (`ScriptFailed`), no metric line
  (`MetricMissing`), or a non-finite or unparsable value (`MetricInvalid`).
  Every stdout line of the form `DIAG <text>` is kept, whether or not the seed
  passed, in `SeedRun::diagnostics`: the last 20 non-empty lines, each cut to
  300 characters with an ellipsis. Diagnostics never affect the score. They
  exist so the supervisor sees what a run did, not only how well.
- **Score:** a candidate with any failed seed is rejected. Otherwise the
  candidate and incumbent per-seed metrics on the identical seeds are mapped to
  qualities with the monotone transform `q = 1/2 + atan(s * m) / pi` (`s = +1`
  to maximize, `-1` to minimize), which preserves order and lands in `(0, 1)`
  as `compare` requires, and compared with `evaluation::compare`. The
  decision is `compare`'s. The trace's `estimate/lower/upper` are reported in
  metric units: the mean paired improvement `s * (candidate - incumbent)` plus
  or minus the policy's standard-error multiple. Under `run_script_loop_with`
  a scorer without metric evidence on both sides reports quality units, and
  the ledger row says so with `units: "quality"` (omitted for metric units).
- **Improve:** among the epoch's accepted candidates the one with the best mean
  metric becomes the incumbent and later proposals build on it.
- **Ideate:** `Supervisor` (now `tinyrsi_evolve::ScriptSupervisor`) wraps a `ModelPort` and makes one forced native
  tool call `propose_script` whose JSON schema is `ScriptProposal { approach,
  differs_from_previous?, rationale, hypothesis, files: [{path, content}] }`.
  Its prompt holds the goal and target, the incumbent's editable sources, the
  per-epoch metric history, the approaches already tried and the earlier
  proposals of this epoch (see the [hypothesis ledger](hypothesis-ledger.md)),
  the last `rejected_context` rejected proposals with their approach, mean
  metric, decision, stderr tail and diagnostics, the incumbent's diagnostics
  for every seed, and last the slot's directive. A slot directive's
  temperature is passed to the model request. A reply that is not exactly one such object is
  `SupervisorMalformed`; a path outside `editable` (including absolute paths
  and `..`) is `ProposalPathOutsideEditable`. Both are recorded as invalid
  attempts and the loop continues. A transport failure ends the run.
- **Stop:** `goal_reached` when the incumbent meets the target (checked after
  the baseline and after each epoch), `stalled` after `patience` epochs without
  promotion, `budget` when measured supervisor (and scorer) tokens reach `token_budget`
  before an ideation call, `max_epochs` otherwise, and `error` when the run
  fails (the error is returned after `run_finished` is written).
- **Ledger:** `<run>/ledger.json`, rewritten atomically (temporary file,
  `fsync`, rename, directory `fsync`) after every candidate, holds every
  candidate with its parent, proposal (including its approach label, any
  `differs_from_previous` and the requested temperature), per-seed runs, mean,
  decision and interval, the incumbent chain, token use and the stop reason.

New errors: `InvalidScriptSubject`, `ScriptBaselineFailed`, `ScriptTimeout`,
`ScriptOutputExceeded`, `ScriptFailed`, `MetricMissing`, `MetricInvalid`,
`ProposalPathOutsideEditable`, `SupervisorMalformed`, `TraceIo`.

## Invariants and constraints

- A candidate is promoted only on an `Accept` from `compare` with zero failed
  seeds, on exactly the seeds the incumbent was measured on.
- The baseline must succeed on every seed, or the run ends with
  `ScriptBaselineFailed`.
- Identical content is never executed twice; a duplicate proposal is rejected.
- The verification module keeps its exact process semantics; the runner is a
  shared private module.

## Safety

LLM-written code runs **locally, with the invoking user's privileges**. The only
constraints are: it runs on a copy of the workspace, with a cleared environment
(no inherited credentials such as `OPENROUTER_API_KEY`), a wall-clock timeout
(the whole process group is killed), and an output cap. There is **no network
sandbox and no filesystem sandbox**: a proposal can read or write anything the
user can and can open network connections. Run live supervision only on a
machine and account where that is acceptable, or inside a container or VM. The
offline example uses canned proposals and never calls a model.

## Acceptance criteria

- A deterministic test drives a scripted supervisor that proposes a better then
  a worse script: the better one is accepted and promoted, the worse one
  rejected, and the trace and ledger record both.
- Every stop reason and every new error variant has a producing test.
- `cargo run -p tinyrsi --example script_loop` runs offline and records at
  least one promotion and one rejection.

## Open questions

None.
