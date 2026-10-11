# Diagnostics feedback: live reruns

Scripts can now print `DIAG <text>` lines, and the supervisor sees them in
its next prompt for the incumbent and for each recent rejection (see the
[script loop spec](../specs/script-loop.md)). This page reruns the live
benchmarks that had stalled
([graph colouring](graph-coloring-results.md), [curve fit](script-loop-results.md))
to see whether that feedback moves them.

Evidence: [`results/2026-10-11/diagnostics/`](results/2026-10-11/diagnostics/)
holds the trace and ledger of each run, with run paths redacted to
`<run-dir>`, plus Sonnet's winning `coloring.py`.

## Results

| Run | Before diagnostics | With diagnostics |
| --- | --- | --- |
| Graph colouring, `claude-sonnet-4.5` | 17 → 13.33, stalled | **17 → 6 (optimum), goal reached in 3 epochs** |
| Graph colouring, `gpt-4.1-mini` | 17 → 13, stalled | 17 → 13, stalled after 4 epochs |
| Curve fit, `gpt-4.1-mini` | 0.644 → 0.258 / 0.413, stalled | 0.644 → 0.458, stalled after 4 epochs |

These are single runs per configuration. Treat the Sonnet result as a strong
signal, not a measured rate.

### Sonnet 4.5 used the diagnostics to debug itself

It instrumented its own code without being told how. The `DIAG` lines it
printed made each failure legible:

- Epoch 2, Kempe chains: `After Kempe optimization: 12 colors … 50
  iterations`. The pass ran and did nothing.
- Epoch 2, iterated greedy: `Failed to find 11-coloring, stopping`, after
  18 s.
- Epoch 3, tabu search: `DSATUR initial: 12 colors`, then `Found
  11-coloring`, `Found 10-coloring`, … down to 6 on every seed. Accepted, goal
  reached.

The previous run, without diagnostics, proposed the same ideas (Kempe,
iterated greedy) and saw them only as "mean 13.33, rejected". It never moved
on to a search method that worked.

A second tabu proposal in the same epoch printed `Failed to find
11-coloring` and was rejected. Same idea, weaker code. Variance in
implementation quality is now the main source of noise.

### gpt-4.1-mini read the diagnostics and did not act on them

Its own lines said `Greedy recoloring reduced to 14 colours with 0 moves`.
It then proposed that same recolouring five more times. On the curve fit, the
residual profile (`+0.46 -0.53 -0.24 +0.64 -0.16 …`) plainly oscillates, but
it kept adding ridge regularization to a cubic. Diagnostics give a capable
supervisor the evidence it needs. They do not make a weak one reason better.

## Conclusions

1. Diagnostics turn "the idea failed" into "the code did X". That is the
   difference between stalling at DSATUR and reaching the optimum.
2. Supervisor capability now dominates. The same loop, prompt and evidence
   succeed with one model and fail with another.
3. Repetition is the remaining loop-side failure. Both weaker runs spent
   most of their proposals resubmitting ruled-out ideas, and proposals within
   an epoch are often identical.

The [improvement roadmap](script-loop-roadmap.md) ranks what to build next.
