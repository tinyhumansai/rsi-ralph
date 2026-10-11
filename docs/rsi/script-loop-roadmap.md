# Script loop: improvement roadmap

What to build next for the plain-script loop, ranked by expected effect per
unit of work. The ranking comes from five live runs on three benchmarks:
[curve fit](script-loop-results.md), [graph colouring](graph-coloring-results.md)
and the [diagnostics reruns](diagnostics-results.md). Each item names the
failure it targets, the change, where it lands in the code, and how to tell
whether it worked. Each item gets its own spec in `docs/specs/` and plan in
`docs/plans/` before code, as the repository requires.

## Observed failures

| Failure | Seen in | Cost |
| --- | --- | --- |
| Supervisor blind to what the code did | all pre-diagnostics runs | stalls at the first plateau; **fixed by `DIAG` lines** for capable models |
| Resubmitting ruled-out ideas | every stalled run (same idea up to 6×) | most of the proposal budget |
| Identical proposals within an epoch | curve fit, graph colouring | halves the useful proposals per epoch |
| Weak supervisor ignores evidence | `gpt-4.1-mini`, all runs | no progress past textbook steps |
| Good idea, weak implementation | Sonnet tabu p3-1, RLF | correct directions rejected |
| Too few seeds to resolve small gains | curve fit degree 7 (inconclusive) | real gains discarded |

## Ranked improvements

### 1. Hypothesis ledger in the prompt (repetition)

**Status: implemented** ([spec](../specs/hypothesis-ledger.md)). `approach`
is required by the tool schema and grouped by normalized label; a resubmitted
approach is not refused, only discouraged, until the benchmark harness
measures the repetition share.

- **Change:** keep a compact, deduplicated list of approaches already tried,
  each with its best metric and a one-line verdict ("Kempe pass: 0 moves, no
  gain"). Ask the supervisor, in the tool schema, to tag each proposal with an
  `approach` label. Render "Already tried" above the recent rejections, and
  tell the supervisor not to resubmit an approach unless it names what is
  different this time.
- **Code:** `ScriptProposal` (tinyrsi-evolve) gains an optional `approach`
  field; `ScriptLedger` stores it; `script_loop/prompt.rs` renders the summary.
- **Measure:** the share of proposals whose approach was already rejected
  (gpt-4.1-mini runs: over 50%). Target under 15%, and fewer epochs to stall.

### 2. Diverse proposals per epoch (identical slots)

**Status: implemented** ([spec](../specs/hypothesis-ledger.md)).
`LoopPolicy::slots` holds the three directives below by default, slots see
earlier slots of the same epoch, and a slot may set a temperature
(`ModelRequest::with_temperature`). The default samples every slot at 0.

- **Change:** give each slot in an epoch a different instruction. For
  example: slot 0 is "best next step", slot 1 is "a different algorithm
  family from the incumbent", slot 2 is "fix or finish the most promising
  rejected idea". Optionally raise the temperature for exploration slots.
  Reject an exact-duplicate reply before running it, as now, and count it.
- **Code:** `ScriptBrief` carries a per-slot directive from `LoopPolicy`;
  `OpenRouterModel` needs an optional temperature (currently fixed at 0).
- **Measure:** distinct approaches per epoch, and epochs to goal on graph
  colouring with Sonnet. The current run took 3 epochs; aim for 2.

### 3. Repair turn for promising failures (weak implementations)

- **Change:** when a candidate fails a seed or ties the incumbent, and its
  diagnostics show the code ran but stopped early ("Failed to find
  11-coloring"), give the same proposal one repair attempt with its own
  diagnostics and stderr. Do this before the next epoch, not as a new idea.
  Cap repairs per epoch, and charge their tokens to the budget.
- **Code:** a `repair` step in `script_loop/mod.rs` between measure and the
  next ideate; ledger rows get a `repair_of` parent link; the trace gains no
  new kinds (a repair is an `ideated` with the failed candidate as `parent`).
- **Measure:** the share of accepted candidates that came from a repair, and
  the token cost per accepted candidate.

### 4. Adaptive seeds for close calls (statistical resolution)

- **Shipped first step:** per-seed dominance (`LoopPolicy::accept_dominant`).
  The benchmark matrix after the hypothesis ledger showed Sonnet reaching 6
  colours three times and the paired interval ruling each one inconclusive
  (`[6, 6, 6]` against `[11, 6, 6]`). A candidate no worse on any seed and
  better on one is now accepted. Extra seeds below remain the fix for gains
  that are uneven in sign.

- **Change:** on `inconclusive`, rerun the candidate and the incumbent on extra
  seeds (up to a cap) before deciding. A sequential test keeps the error rate
  honest.
- **Code:** `LoopPolicy::max_seeds`; `score.rs` re-decides with the larger
  sample; reuse the paired machinery in `tinyrsi-score`.
- **Measure:** the inconclusive rate. Gains below the current resolution
  (curve fit degree 7) should now resolve either way.

### 5. Model tiering (weak supervisors)

- **Change:** run cheap proposals with a small model. Escalate to a strong
  one after `k` epochs without gain, or for repair turns. Record the model per
  proposal in the ledger and the trace.
- **Code:** `ScriptSupervisor` over two `ModelPort`s and a switch policy;
  `ideated` events gain an optional `model`, which is an additive schema
  change that the visualizer shows.
- **Measure:** tokens and cost to reach the goal against Sonnet-only and
  mini-only on all three benchmarks.

### 6. Benchmark harness for the loop itself

**Implemented:** [loop benchmarks spec](../specs/loop-benchmarks.md),
`cargo run -p tinyrsi-examples --example benchmarks`. The repetition share is
a proxy (identical content or normalized hypothesis of an earlier ruled-out
proposal) until item 1 adds an `approach` label.

- **Change:** a runner that executes every benchmark × model × N repeats and
  writes one comparison table: goal rate, epochs and tokens to goal, and the
  repetition share. This turns the single-run anecdotes above into measured
  rates, and gives every item in this list an acceptance number.
- **Code:** a new `examples/benchmarks` binary over the existing examples;
  results under `docs/rsi/results/<date>/`.
- **Build this first, or alongside item 1.** Without it, items 1–5 cannot be
  compared fairly, because one run per configuration is too noisy.

## Also worth doing

- **Read-only context files.** Let a subject list fixed files, such as the
  checker, that the supervisor may read but not edit. Item 1 matters more,
  but this is small.
- **Langfuse for script runs.** Attach `LangfuseTrace` beside `JsonlTrace` in
  the examples when `LANGFUSE_*` is set, so live runs show up next to agent
  traces.
- **Visualizer.** Show `DIAG` lines per candidate in `apps/rsi-viz`. They are
  in `ledger.json` today, but not in the trace.
- **Harder benchmarks.** Graph colouring is now solved by one model in 3
  epochs. Add a larger or less structured instance, such as DIMACS-style
  graphs or a hidden 8-colouring at lower density, so there is headroom to
  measure items 1–5.

## Suggested order

1. Benchmark harness (6), so every later change has a number.
2. Hypothesis ledger (1) and diverse slots (2): one spec, since both change
   the prompt and the schema.
3. Repair turn (3).
4. Adaptive seeds (4) and model tiering (5).
