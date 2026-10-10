# Spec: OpenHuman RSI Framework (draft v0)

Status: draft, pre-implementation. Feedback wanted before anything here is
built. Companion docs: `../rsi-survey.md`, `../openhuman-substrate.md`.

## 1. Purpose

A framework in which OpenHuman agents improve their own harness configuration
through a recursive self-improvement loop: propose config changes, evaluate
candidates on a selection benchmark under fixed budgets, accept only changes
that improve a private held-out grade, and iterate. The model stays fixed;
what evolves is the environment around it.

Non-goals: weight training, modifying OH core code, evolvable guardrails.

## 2. Definitions

- **Environment** — the total mutable surface: prompts and agent definitions,
  skills, workflows, memory policies, tool routing and model routing config.
  Versioned as a whole; a candidate is one full snapshot of it.
- **Candidate** — an environment snapshot plus its provenance (parent, diff,
  proposal rationale).
- **Grade** — aggregate private score of a candidate across the selection
  benchmark, multi-seed.
- **Incumbent** — the candidate currently driving the live configuration.
- **Loop** — the outer optimization process: propose → evaluate → accept/reject
  → record. (We deliberately do *not* ship an inner code-optimization loop
  like the paper's in v0; OH tasks are mostly harness-shaped already. See §8.)

## 3. Architecture

Three OH sub-agents and one workflow, all generic and configurable:

1. **Proposer** (outer-loop agent): reads the incumbent environment snapshot,
   the lineage record, and prior rejected proposals; emits one candidate diff
   plus a rationale. Strongest model, pinned.
2. **Candidate runner** (inner agent): executes the selection tasks under the
   candidate's environment, in isolation, under a fixed budget.
3. **Reviewer**: scores each run against held-out data; extracts score +
   structured feedback (which the Proposer consumes next round — the
   natural-language "gradient" from the TextGrad/GEPA line).
4. **Loop workflow** (tinyflows): the driver. Steps: snapshot incumbent →
   propose → shadow-run candidates → grade → apply acceptance rule → commit
   or reject → append lineage → checkpoint. Scheduled or on-demand.

## 4. The acceptance rule (the load-bearing decision)

Following the paper: candidate accepted iff `grade(candidate) > grade(incumbent)
+ margin`, where `margin` is tuned to observed seed-to-seed noise (start with
2 standard errors of the benchmark mean, re-estimated continuously).

- Inner visible signal and outer acceptance signal stay decoupled: candidates
  and the Proposer see public/run metrics only; the private grade is computed
  by the Reviewer and stored in the lineage record.
- Every rejection is logged with its public-vs-private delta; the rate of
  "public better, private rejected" is a health metric for the loop.
- All evaluations run under identical budget caps; gains must be efficiency,
  not spend.

## 5. Selection benchmark

A configurable set of task families, each with: public task instances, a
private held-out scorer, a budget cap, and N seeds per task. v0 candidates
(task families TBD with the user — this is the biggest open design decision):

- **Harness engineering**: tasks where the object of optimization is agent
  behavior itself (the paper found the largest and most durable gains here).
- **Knowledge-work automation**: multi-step research/synthesis tasks graded on
  extracted artifacts.
- **Coding/engineering**: repo-level tasks with test-based held-out scoring.

Diversity across families is a requirement, not a nicety: it is what forces
the loop toward general mechanisms (the paper's central experimental choice).

## 6. Lineage and memory

A dedicated memory subtree (`rsi/`) holds: environment snapshots
(content-addressed), per-candidate grade records, proposal rationales,
rejected-proposal archive, and a rolling summary the Proposer reads. The
rejected archive matters — Mendel GFM's lesson is that comparative signals
across attempts are themselves learning signal.

## 7. Safety

- **Frozen surfaces**: approval gates, permissions, secrets/keyring, memory
  hygiene rules, and this spec's own constraints are outside the environment;
  the Proposer's diff tooling must reject any diff touching them.
- **Shadow mode default**: candidates run isolated; the live configuration
  changes only after acceptance, and the first M accepted rewrites require
  human approval via OH gates.
- **Revertibility**: every accepted state has a one-command revert to its
  parent snapshot.
- **Canary metrics**: reward-hacking rate and cost-per-task are tracked every
  round even though the grade doesn't optimize them (the paper's emergent
  reward-hacking reduction shows what can drift; drift in either direction
  is information).

## 8. Phasing

1. **P0 — instrument**: snapshot/restore of OH environment config; lineage
   store; manual grading workflow. No loop.
2. **P1 — single loop, human-gated**: Proposer + shadow evaluation +
   acceptance rule + approval gate on commit. Small benchmark (2 families).
3. **P2 — autonomous loop** in shadow mode with periodic human review;
   multi-seed grading; canary metrics dashboard.
4. **P3 — ignition**: promote an accepted agent to Proposer and compare
   sample-efficiency against the original, per the paper's ignition test.

## 9. Open decisions

1. Task families and concrete benchmarks for the selection set (§5) — drives
   everything downstream; needs the user's answer on what the OH agent should
   get better *at*.
2. Config snapshot mechanics in OH (export/import coverage, DB vs files) —
   marked *verify* in the substrate doc; a P0 spike.
3. Compute budget per round; the paper spent 8 days of frontier-model spend.
4. Whether skills are mutable in v1 or frozen until the loop is trusted.
