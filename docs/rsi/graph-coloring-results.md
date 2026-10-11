# Graph colouring: live supervisor runs

Two live runs of the [`graph_coloring`](../../examples/graph_coloring/main.rs)
example, asking whether the script loop can take a graph-theory solver from a
naive heuristic to the optimum when an LLM writes every rewrite.

**Short answer:** no, not yet. Both supervisors climbed the textbook greedy
ladder and then stalled at about 13 colours, against an optimum of 6. The
canned offline run (DSATUR followed by TabuCol local search) shows the target
is reachable in about 1.5 s per graph. The loop's checks all held: invalid
colourings failed their seeds, ties were rejected and every promotion was
backed by a measured gain.

Traces: [`results/2026-10-11/graph-coloring/`](results/2026-10-11/graph-coloring/).

## Problem

`check_coloring.py` (fixed) builds a 150-vertex graph per seed with a hidden
6-colouring. Pairs from different secret classes are joined with probability
0.35, about 3,300 edges in all. Labels and edge order are shuffled, and the
seed is removed from the environment before the solver is imported.
`coloring.py` (editable) must return a proper colouring. The metric is the
number of colours, which is minimized with target 6. The seeds are 1, 2 and 3.
The policy is ≤ 6 epochs × 2 proposals, with patience 3.

Reference points on these graphs (colours for seeds 1/2/3):

| Solver | Colours | Time |
| --- | --- | --- |
| first fit in label order (start) | 17 / 18 / 16 | < 0.1 s |
| DSATUR | 14 / 12 / 13 | < 0.1 s |
| DSATUR + TabuCol | 6 / 6 / 6 | ≈ 1.5 s |

## Runs

| Supervisor | Path | Result | Proposals | Stop |
| --- | --- | --- | --- | --- |
| `openai/gpt-4.1-mini` | first fit → Welsh–Powell (15) → DSATUR (13) | 17 → 13.0 | 10; 4 accepted, 6 rejected | stalled after 5 epochs |
| `anthropic/claude-sonnet-4.5` | first fit → DSATUR (13.33) | 17 → 13.33 | 8; 2 accepted, 6 rejected | stalled after 4 epochs |

- **gpt-4.1-mini** found DSATUR in two steps. It then spent three epochs on
  the same micro-optimisation ("avoid processing stale heap entries"), six
  near-identical proposals that could not change the colouring. It never
  tried local search, backtracking or Kempe chains.
- **Sonnet 4.5** jumped straight to DSATUR and then proposed the right family
  of ideas: Kempe-chain recolouring, iterated greedy with backtracking,
  independent-set extraction, and RLF. None of them helped. The Kempe and
  iterated-greedy versions produced the same per-seed totals as DSATUR, so
  their refinement passes moved nothing. RLF was implemented badly and got
  worse (19.7). The supervisor saw only the mean colour count, so it could not
  tell "the idea is wrong" from "my code never fires".

## What this shows about the loop

1. **Execution feedback is the bottleneck, not ideas.** The stronger model
   named the right algorithms. What it lacked was evidence about its own
   code: whether the refinement ran, how many recolourings it tried, and where
   it got stuck. The checker emits a single scalar. This is the same
   limit the curve-fit run hit ([script loop results](script-loop-results.md)).
2. **Proposals within an epoch are near-duplicates.** Both slots get the same
   prompt at temperature 0, so the second proposal rarely explores.
3. **No memory of ruled-out ideas.** The prompt lists recent rejections, but
   gpt-4.1-mini still resubmitted one idea six times.

The ranked fixes are the ones in the script-loop report: subject diagnostics
in the prompt (`DIAG` lines, e.g. conflicts remaining and moves tried), a
summary of hypotheses already tried, and diversity across proposal slots.
This example is a good benchmark for them because the gap to the optimum is
large, the checker is exact, and every run is cheap.
