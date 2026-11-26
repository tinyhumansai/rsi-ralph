# Search strategies for the script loop: hill climbing, restarts, HGM

Recorded 2026-11-26. Which version should each proposal edit, and what should
the loop do when progress stops? This page compares four answers on the live
benchmark harness: 220 independent runs, about 13.6M measured tokens.
Aggregate receipts (no paths, prompts or sources) are in
[`results/2026-11-26/search-strategies.json`](results/2026-11-26/search-strategies.json).

**Short answer.**

- **Not stopping early is the change that matters.** The benchmarks' default
  policies stop after 3 epochs without a promotion. Giving the same hill
  climber 10 epochs and no stall stop doubled the goals reached, from 6/20
  to 12/20 on the same 5-run design:
  - graph colouring: 4/10 to 5/10;
  - curve fit: 2/10 to 7/10.
- **How the loop picks what to edit next made no significant difference** at
  a 10-epoch budget. Restarts, three variants of Huxley-Gödel Machine (HGM)
  tree search and a hill/HGM hybrid all landed within noise of plain hill
  climbing.
- **The hybrid is the best-behaved exploration mode.** It reached the most
  goals (15/30 against hill-10's 13/30) and was never worse than hill
  climbing in any cell, but the difference is not significant (Fisher
  p = 0.80, progress p = 0.59).

## What was compared

All modes are `LoopPolicy::search` values ([spec](../specs/script-loop.md)).
In every mode a candidate becomes the champion only by winning the paired
test against it, so the reported result never gets worse.

| Mode | Proposals edit | When stalled (`patience` quiet epochs) |
| --- | --- | --- |
| `Hill` (default) | the champion | stop as `stalled` |
| `Restart` | the champion, or a restart branch | branch to the best earlier candidate whose per-seed metrics differ from every parent's; a stalled branch returns to the champion |
| `Hgm` | a node chosen per proposal by Thompson sampling over clade records, with cooling | never stops on a stall |
| `Hybrid` | the champion; a detour after a stall | detour to an HGM-chosen node outside the champion chain; a stalled detour returns to the champion |

`Hgm` follows Wang et al.'s Huxley-Gödel Machine and its reference code
(`TS_sample` over a node and its descendants). Each draw is
`Beta(tau (1 + s), tau (1 + f))`, cooled from `tau = 1` in epoch 1 to
`tau = max_epochs` in the last.

HGM counts benchmark-task passes. A script reports a continuous metric, so
it needs a success signal. Three were tried; the last is committed:

| Version | A seed counts as a success when it… | Problem found |
| --- | --- | --- |
| HGM v1 (no cooling) | makes progress from the baseline to the target, fractionally | saturates near the target: every good node scores about 1 |
| HGM v2 | beats its parent | favours weak lines, which are easy to improve |
| **HGM v3 (committed)** | beats the champion of its time | the champion's own line accumulates failures (see below) |

Restart v1 never went back to the champion after branching. That hurt curve
fit with gpt-4.1-mini (2/5 against hill-10's 4/5), and returning to the
champion (v2, committed) fixed it. Superseded versions are kept in the
receipts and marked as such.

## Method

- **Harness:** `cargo run -p rsi-ralph-examples --example benchmarks` with
  `TINYRSI_LIVE_OPENROUTER=1` and the new `--search`, `--epochs` and
  `--patience` flags.
- **Benchmarks:**
  - `graph_coloring`: 150 vertices with a hidden 6-colouring, starting
    at 17 colours.
  - `curve_fit`: target RMSE 0.06, starting at 0.644.
  - Both use three seeds per candidate and two proposals per epoch.
- **Models:** `openai/gpt-4.1-mini` (weak) and `anthropic/claude-sonnet-4.5`
  (strong), each as both supervisor and the only proposer.
- **Budget:** 10 epochs for every compared arm. Hill-10 sets patience 10, so
  it never stops early.
- **Runs:** 5 independent runs per cell. The cell where search could matter
  most, graph colouring with gpt-4.1-mini, got 15.
- **Progress:** `(start - final) / (start - target)`, capped at 1.
- **Tests:** two-sided permutation tests on progress, and Fisher's exact
  test on goals.

## Results

Every 10-epoch arm ran every cell: 15 runs for graph colouring with
gpt-4.1-mini (where hill climbing plateaus), 5 for the others. "Hill, default
policy" is the benchmark's own policy (graph colouring 6 epochs, curve fit 4,
patience 3) for reference; the p-values compare each arm's progress with
hill-10 (two-sided permutation test). HGM v1 and v2 are superseded versions,
kept for the record. A curve-fit goal is an incumbent mean at or below 0.06.

**Graph colouring (start 17 colours, optimum 6), gpt-4.1-mini**

| Arm | Runs | Goals | Mean progress | p vs hill-10 | Final results | Median tokens/run |
| --- | --- | --- | --- | --- | --- | --- |
| Hill, default policy | 5 | 0/5 | 0.47 | — | 7, 13, 13, 13, 13 | 20k |
| Hill-10 (baseline) | 15 | 1/15 | 0.54 | — | 6, 7, 7.7, 8, 11, 11.3, 11.7, 13, 13, 13, 13, 13, 13, 13, 13 | 62k |
| Restart | 15 | 1/15 | 0.63 | 0.32 | 6, 6.7, 6.7, 7, 7.3, 8, 10.7, 11, 11.3, 12, 13, 13, 13, 13, 13 | 64k |
| HGM v1 (superseded) | 15 | 2/15 | 0.57 | 0.78 | 6, 6, 7, 7, 8, 11.7, 12, 13, 13, 13, 13, 13, 13, 13, 13 | 65k |
| HGM v2 (superseded) | 15 | 1/15 | 0.54 | 1.00 | 6, 7.3, 8, 8.7, 11, 11.3, 11.7, 11.7, 12.7, 13, 13, 13, 13, 13, 13 | 63k |
| HGM v3 (committed) | 15 | 0/15 | 0.55 | 0.85 | 7, 7, 8, 8, 9, 11, 11, 11.7, 13, 13, 13, 13, 13, 13, 13 | 61k |
| Hybrid | 15 | 0/15 | 0.61 | 0.41 | 7, 7, 7.7, 8, 8, 8.7, 10, 11, 11.3, 11.3, 13, 13, 13, 13, 13 | 63k |

**Graph colouring (start 17 colours, optimum 6), Sonnet 4.5**

| Arm | Runs | Goals | Mean progress | p vs hill-10 | Final results | Median tokens/run |
| --- | --- | --- | --- | --- | --- | --- |
| Hill, default policy | 5 | 4/5 | 0.96 | — | 6, 6, 6, 6, 8 | 55k |
| Hill-10 (baseline) | 5 | 5/5 | 1.00 | — | 6, 6, 6, 6, 6 | 64k |
| Restart | 5 | 4/5 | 0.95 | 1.00 | 6, 6, 6, 6, 8.7 | 45k |
| HGM v1 (superseded) | 5 | 4/5 | 0.96 | 1.00 | 6, 6, 6, 6, 8.3 | 33k |
| HGM v2 (superseded) | 5 | 5/5 | 1.00 | 1.00 | 6, 6, 6, 6, 6 | 51k |
| HGM v3 (committed) | 5 | 4/5 | 0.89 | 1.00 | 6, 6, 6, 6, 12 | 79k |
| Hybrid | 5 | 5/5 | 1.00 | 1.00 | 6, 6, 6, 6, 6 | 44k |

**Curve fit (start RMSE 0.644, target 0.06), gpt-4.1-mini**

| Arm | Runs | Goals | Mean progress | p vs hill-10 | Final results | Median tokens/run |
| --- | --- | --- | --- | --- | --- | --- |
| Hill, default policy | 5 | 0/5 | 0.98 | — | 0.061, 0.070, 0.070, 0.073, 0.081 | 23k |
| Hill-10 (baseline) | 5 | 4/5 | 1.00 | — | 0.053, 0.054, 0.054, 0.056, 0.060 | 58k |
| Restart | 5 | 4/5 | 1.00 | 1.00 | 0.053, 0.054, 0.058, 0.059, 0.073 | 58k |
| HGM v1 (superseded) | 5 | 1/5 | 0.99 | 0.05 | 0.054, 0.064, 0.067, 0.070, 0.073 | 74k |
| HGM v2 (superseded) | 5 | 3/5 | 0.99 | 0.45 | 0.053, 0.054, 0.054, 0.066, 0.070 | 56k |
| HGM v3 (committed) | 5 | 1/5 | 0.99 | 0.05 | 0.054, 0.062, 0.067, 0.070, 0.070 | 72k |
| Hybrid | 5 | 5/5 | 1.00 | 1.00 | 0.053, 0.054, 0.054, 0.055, 0.059 | 52k |

**Curve fit (start RMSE 0.644, target 0.06), Sonnet 4.5**

| Arm | Runs | Goals | Mean progress | p vs hill-10 | Final results | Median tokens/run |
| --- | --- | --- | --- | --- | --- | --- |
| Hill, default policy | 5 | 2/5 | 0.99 | — | 0.054, 0.054, 0.067, 0.070, 0.075 | 51k |
| Hill-10 (baseline) | 5 | 3/5 | 0.99 | — | 0.053, 0.054, 0.057, 0.066, 0.078 | 42k |
| Restart | 5 | 4/5 | 1.00 | 0.45 | 0.053, 0.057, 0.057, 0.057, 0.061 | 44k |
| HGM v1 (superseded) | 5 | 4/5 | 1.00 | 0.72 | 0.054, 0.054, 0.054, 0.059, 0.074 | 27k |
| HGM v2 (superseded) | 5 | 4/5 | 1.00 | 0.45 | 0.054, 0.055, 0.059, 0.060, 0.060 | 46k |
| HGM v3 (committed) | 5 | 4/5 | 1.00 | 0.45 | 0.053, 0.054, 0.056, 0.059, 0.060 | 72k |
| Hybrid | 5 | 5/5 | 1.00 | 0.45 | 0.053, 0.054, 0.060, 0.060, 0.060 | 46k |

**All four cells pooled (10-epoch arms, 30 runs each)**

| Arm | Goals | Goals p (Fisher) | Mean progress | Progress p | Total tokens |
| --- | --- | --- | --- | --- | --- |
| Hill-10 (baseline) | 13/30 | — | 0.766 | — | 1.96M |
| Restart | 13/30 | 1.00 | 0.804 | 0.59 | 1.95M |
| HGM v1 (superseded) | 11/30 | 0.79 | 0.773 | 0.93 | 1.93M |
| HGM v2 (superseded) | 13/30 | 1.00 | 0.768 | 0.98 | 1.90M |
| HGM v3 (committed) | 9/30 | 0.42 | 0.757 | 0.89 | 2.05M |
| Hybrid | 15/30 | 0.80 | 0.803 | 0.59 | 1.88M |

No difference between any 10-epoch arm and hill-10 is significant, in any
cell or pooled. The only cells with p at or below 0.05 are curve fit with
gpt-4.1-mini for HGM v1 and v3, and they go *against* HGM: its runs ended just
above the 0.06 target, where hill's ended just below.

**Why Sonnet and gpt-4.1-mini differ.** Sonnet reaches the goal in 2 to 5
epochs in most runs, usually before any 3-epoch stall, so restart and hybrid
rarely diverge from hill climbing with it, and every arm sits at 4/5 or 5/5.
gpt-4.1-mini reaches DSATUR (13 colours) in epoch 1 and then plateaus, which
is where the strategies differ, so that cell got 15 runs.

### Where the stalls are, and what rescues them

Over the 30 hill-10 runs, 21 had a stretch of 3 or more epochs without a
promotion, which is where the default policy stops.

- **Graph colouring, gpt-4.1-mini:** 14 of 15 runs plateaued, nearly all at
  13 colours (DSATUR) by epoch 4. Continuing to hill climb rescued only 3.
- **Overall:** 8 of the 21 plateaus recovered by simply continuing. That is
  why the stall stop costs so much.

Restarts and detours do rescue some plateaus, as the ledgers' parent links
show:

| Mode | Runs that used a branch | Champion promotions from branches |
| --- | --- | --- |
| Restart, graph colouring, mini | 15 | 6 runs improved via a branch, including 13 to 6.7 twice and 7 to 6 (optimum) |
| Hybrid, graph colouring, mini | 15 (20 detours) | 2 (13 to 8, 13 to 11.3) |
| Hybrid, curve fit, mini | 1 (1 detour) | 1 (0.07 to 0.054) |
| Hybrid, graph colouring, Sonnet | 1 (1 detour) | 0 |
| Hybrid, curve fit, Sonnet | 1 (1 detour) | 1 (0.165 to 0.06) |

Removing the branch-derived promotions from the restart runs brings their
mean progress from 0.63 to 0.50, level with hill-10's 0.54. So the restart
edge comes entirely from branches, but it is small (+0.13 attributed,
p of about 0.07).

The first 5 runs of this cell suggested a large effect: all five hill runs
stayed at 13. The next 10 hill runs escaped 13 in 7 cases. Small samples
mislead here.

### Why pure HGM does not help at this budget

HGM edits a node other than the champion most of the time, and more so late
in the run. HGM v3 with graph colouring and gpt-4.1-mini:

- epochs 1 to 5: 86 of 150 proposals were off the champion;
- epochs 6 to 10: 128 of 150.

Cooling worked as designed: it commits to the clade with the best record.
That clade is rarely the champion's, for two reasons:

- **The champion's line builds up failures.** Near the best, almost every
  attempt to improve the champion fails, whatever counts as a success.
- **Fresh nodes look competitive.** A node with no record draws from
  `Beta(tau, tau)`, centred on one half.

With hundreds of expansions that exploration finds better lineages. The
reference configuration budgets 800 task evaluations. With 20 proposals per
run it takes attempts away from polishing the best line. The hybrid avoids
this by exploring only after a stall.

## Recommendations

1. **Raise the stall stop.** Prefer more epochs with a larger `patience`
   (or no stall stop at all) over the current 2 to 3 epochs. This is the only
   change here with a clear effect.
2. **Keep hill climbing as the default.**
3. **Offer `Hybrid` as the exploration option.** It reached the most goals,
   used the fewest tokens of the 10-epoch arms (1.88M against 1.96M) and was
   never worse than hill climbing in any cell.
4. **Treat `Restart` and `Hgm` as experimental.** `Hgm` probably needs a
   budget an order of magnitude larger, which these runs did not test.

## Limitations

- **Two small benchmarks.** Sonnet usually solves both within 2 to 5 epochs,
  before any strategy diverges from hill climbing. Harder or pass/fail task
  suites, like those HGM and DGM use, may behave differently.
- **Small samples.** There are 5 runs in most cells, so only large effects
  are detectable.
- **Feedback and promotion share seeds.** The same three seeds feed `DIAG`
  feedback and decide promotion, a reuse that REUSE warns can overfit. Final
  results were not re-scored on held-out seeds.
- **Short budget.** 10 epochs of 2 proposals. HGM's advantage in the
  literature comes at a far larger scale.
- **Run order.** Superseded variants ran before their replacements, and
  the Sonnet hybrid cells ran last, on the same day and harness.

## Reproduce

```sh
TINYRSI_LIVE_OPENROUTER=1 cargo run -p rsi-ralph-examples --example benchmarks -- \
  --benchmarks graph_coloring --models openai/gpt-4.1-mini --repeats 15 --jobs 5 \
  --search hybrid --epochs 10 --patience 3 --out target/rsi-benchmarks/gc-mini-hybrid
```

- **Other arms:** swap `--search hill --patience 10`, `--search restart
  --patience 3` or `--search hgm`.
- **Default policy:** omit `--search`, `--epochs` and `--patience`.

## References

Numbers quoted from these works are their authors' own.

- G. Huntley, "Ralph Wiggum as a 'software engineer'", 2025.
  <https://ghuntley.com/ralph/>. The original fixed-prompt outer loop.
- J. Zhang, S. Hu, C. Lu, R. Lange, J. Clune, "Darwin Gödel Machine:
  Open-Ended Evolution of Self-Improving Agents", 2025.
  <https://arxiv.org/abs/2505.22954>. Archive-based self-modification; also
  reports objective hacking when the agent could see its checker.
- W. Wang, P. Piękos, …, J. Schmidhuber, "Huxley-Gödel Machine", 2025.
  <https://arxiv.org/abs/2510.21614>, code
  <https://github.com/metauto-ai/HGM>. Clade metaproductivity, Thompson
  sampling and cooling, implemented here as `Search::Hgm`.
- "Group-Evolving Agents", 2026. <https://arxiv.org/abs/2602.04837>. Group
  evolution with performance and novelty selection.
- J. Zhang et al., "Hyperagents", 2026. <https://arxiv.org/abs/2603.19461>.
  An editable improvement procedure. The authors do not claim accelerating
  improvement.
- "Agentic Harness Engineering", 2026. <https://arxiv.org/abs/2604.25850>.
  Harness components as files under git, with rollback of edits that do not
  help.
- Iacob et al., "Red Queen Gödel Machine", 2026.
  <https://arxiv.org/abs/2606.26294>. Evaluators that co-evolve under
  ground-truth gates.
- "LoopsBench", 2026. <https://arxiv.org/abs/2608.00267>. Measures Ralph and
  goal-mode loops on long-horizon coding.
- Srikanth et al., "AIDE²: Recursive self-improvement of AI research
  agents", 2026. <https://arxiv.org/abs/2609.26457>. A single incumbent with
  a private held-out gate, the same shape as this loop's champion rule.
- Sun et al., "Which Self-Improvements Should We Trust? (REUSE)", 2026.
  <https://arxiv.org/abs/2609.33180>. Reusing a promotion set overfits;
  certified promotion.
- Chen, Zhu, Zheng, "Auditing the Product, Not the Recursion", 2026.
  <https://arxiv.org/abs/2610.27610>. Asks for repeated full-loop runs and
  budget-matched comparisons of improvement procedures, the standard this
  page aims at.
- Shah, Kathrotia, "Harnesses That Improve Themselves", 2026.
  <https://arxiv.org/abs/2611.22299>. Gated per-module harness updates; the
  harness must not control its own permissions, grading or logs.
