# Search strategies for the script loop: hill climbing, restarts, HGM

Recorded 2026-11-26. Which version should each proposal edit, and what should
the loop do when progress stops? This page compares four answers on the live
benchmark harness: 210 independent runs, about 12.9M measured tokens.
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
- **The hybrid is the safest exploration mode.** Its trend is slightly
  positive and it was never worse than hill climbing. The evidence is weak.

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

Each cell shows goals reached and mean progress; `n` is runs per arm.

| Arm | Graph colouring, mini (n=15) | Graph colouring, Sonnet (n=5) | Curve fit, mini (n=5) | Curve fit, Sonnet (n=5) |
| --- | --- | --- | --- | --- |
| Hill, default policy (n=5) | 0/5, 0.47 | 4/5, 0.96 | 0/5, 0.98 | 2/5, 0.99 |
| **Hill, 10 epochs** | 1/15, 0.54 | 5/5, 1.00 | 4/5, 1.00 | 3/5, 0.99 |
| Restart | 1/15, 0.63 | 4/5, 0.95 | 4/5, 1.00 | 4/5, 1.00 |
| HGM v1 | 2/15, 0.57 | 4/5, 0.96 | 1/5, 0.99 | 4/5, 1.00 |
| HGM v2 | 1/15, 0.54 | 5/5, 1.00 | 3/5, 0.99 | 4/5, 1.00 |
| HGM v3 | 0/15, 0.55 | 4/5, 0.89 | 1/5, 0.99 | 4/5, 1.00 |
| Hybrid | 0/15, 0.61 | not run | 5/5, 1.00 | not run |

Totals over the four cells, each against hill-10's 13/30 and 0.766:

| Arm | Goals | Mean progress | p (progress) |
| --- | --- | --- | --- |
| Restart | 13/30 | 0.804 | 0.59 |
| HGM v1 | 11/30 | 0.773 | 0.93 |
| HGM v2 | 13/30 | 0.768 | 0.98 |
| HGM v3 | 9/30 | 0.757 | 0.90 |

The hybrid ran only the two gpt-4.1-mini cells:

- graph colouring: progress 0.61 against 0.54 (p = 0.41); final colouring
  below 13 in 10/15 runs against 7/15 (p = 0.46);
- curve fit: 5/5 against 4/5 goals.

No difference between any 10-epoch arm and hill-10 is significant. The only
cell with p < 0.05 is curve fit with gpt-4.1-mini, and it goes *against*
HGM v1 and v3: their runs ended just above the 0.06 target, where hill's
ended just below.

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
3. **Offer `Hybrid` as the exploration option.** It is cost-neutral and was
   never worse than hill climbing in these runs.
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
- **Uneven coverage.** The hybrid was run only with gpt-4.1-mini, and
  superseded variants were run before their replacements.

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
