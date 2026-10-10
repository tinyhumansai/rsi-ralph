# TinyRSI hive orchestration

Related: [umbrella #1](https://github.com/tinyhumansai/tinyrsi/issues/1),
[scoring #2](https://github.com/tinyhumansai/tinyrsi/issues/2).

## Improvement object

TinyRSI is an external host process improving a tinyhivemind `Package`. It edits
only typed `Patch` operations and validates through tinyhivemind-lang's atomic
`apply` and constitution `guard`. It does not rewrite OpenHuman configuration
or model weights. Candidate identity includes the exact package and the hive
memory watermarks. The language `Record` is reused verbatim as a wire type.

## Epoch

1. Observe coordinator changes with `subscribe`, `episodes`, `read_transcript`.
   `Observer` projects message identifiers and episode state; it discards text.
2. Diagnose using `Dossier::build`, external verifier outcomes, measured seat
   usage and memory-health counters. The model cannot invent these counts.
3. Retreat receives public evidence and redacted hive documents, plus labelled
   previous attempts. `build_retreat_request` targets tinybackrooms' generation
   API. Its output is a strict typed `Proposal`, not an instruction to execute.
4. Formalize, guard and deduplicate. Require a positive predicted gain, existing
   dossier nodes and actual transcript references. Freeze verifier criteria.
5. Screen, select and confirm in isolated shadow runs. Each paired arm uses
   identical task IDs, seeds, per-run budgets and frozen private verifiers.
   Confirmation must use fresh task IDs and fresh seeds. The private harness
   sends measurements rather than oracle answers to the controller.
6. Require statistical separation and no family, health or cost regression.
   Request human approval for the exact candidate identity.
7. Record proposal reasons, exact per-task evidence, package, memory watermarks
   and the language record durably. Unscored candidates retain prepared snapshots.
8. Return the accepted design and host activation boundary. Context/memory are
   next-turn deltas; prompt replaces an agent; roster uses host management;
   topology, policy and goals require between-session restart. The library does
   not silently mutate a live coordinator.

One class is chosen per epoch. The class order is context, memory, prompt,
roster, topology, policy and goal. All live promotions remain human-gated.
One-step `Archive::revert` restores the accepted parent design pointer without
deleting historical decisions; the host must restore that package and memory.

## Verification discipline

[way-of-working.md](way-of-working.md) defines the runtime blueprint. A host
verifier reports each node outcome. A completion claim is diagnostic and never
earns partial-credit points. Only eval-owned checkpoints contribute to quality.
A proof kernel, deterministic execution, schema, private reference, calibrated
rubric and preference form the verifier strength ladder. Weak evidence remains
visible to the reader.

## Memory and protected surfaces

Run memory is episodic. Hive memory changes only through approved, versioned
learning deltas. Orchestrator and retreat namespaces are separate from seats.
Evaluation namespaces, private tasks, budgets, telemetry, constitution and
verifier settings are frozen. Held-out canaries must never reach a proposal
model, a package document or hive learning. The language guards package/learning
content; the host scans the actual memory store.

`Evaluator::pin_memory` must fork an isolated snapshot, enforce learning entry
preconditions and expose a complete watermark map. Shared mutable memory cannot
produce a reproducible evaluation. The host retains every snapshot until its
design is no longer needed for replay or rollback.

## Scoring

[scoring.md](scoring.md) defines family-normalized `H`, bootstrap uncertainty,
health penalties, hard constraints and frozen anchor progress. `compare` is a
smaller raw-quality paired heuristic for early host integrations. It does not
replace `compare_h` or the selection/anchor split in a scientific experiment.

The selection suite starts with deterministic code, formal maths, private
informal maths/research and harness-engineering families. Humans curate oracle
data and references. A tiny arithmetic smoke suite tests wiring only.

## Phase boundaries

P0 is specifications, citation audit and upstream verifier API requirements.
P1 is read-only observation and durable evidence. P2 is human-gated shadow
selection. P3 requires live completion gating and real Lean/code verifier
execution in the host. P4 needs a running tinybackrooms backend and durable
proposal memory. P5 needs host roster/replacement/restart activation. P6 is an
experiment comparing original and evolved orchestrators on improvement per
unit cost, with several independent trials. API wiring cannot prove ignition.

The library supplies P1/P2 components and external blueprint, scoring and
retreat contracts. Live host integration, curated benchmarks, durable memory
backends and ignition experiments remain separate work in their owning repos.
