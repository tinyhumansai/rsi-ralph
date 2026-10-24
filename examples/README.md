# RSI Ralph examples

Four end-to-end runs of the improvement loop. Each one proposes changes
(evolve), measures and judges them (score), and keeps only what is better
(the loop). Each writes its live trace for the visualizer. All four run
offline by default, with canned model replies and scripted judges, so they
need no network or credentials and run in CI.

| Example | Subject | Evolve | Score |
| --- | --- | --- | --- |
| [`math_problem`](math_problem/main.rs) | `pi.py`, which must compute π to 12 digits | script supervisor rewrites the method | correct digits (metric) + an LLM judge of the method's justification |
| [`upgrade_file`](upgrade_file/main.rs) | `slug.py`, which must meet a written coding standard | script supervisor rewrites the file | standard violations, with behavior tests as a gate (metric) + an LLM judge reading the source |
| [`graph_coloring`](graph_coloring/main.rs) | `coloring.py`, which must colour random graphs with a hidden 6-colouring using as few colours as possible | script supervisor rewrites the algorithm | colours used (metric); an invalid colouring fails the seed |
| [`hive_goal`](hive_goal/main.rs) | a tinyhivemind hive whose agents must solve private tasks | proposals of typed hive patches | paired private-suite evaluation (normalized H) + an approval gate |

```sh
cargo run -p rsi-ralph-examples --example math_problem
cargo run -p rsi-ralph-examples --example upgrade_file
cargo run -p rsi-ralph-examples --example graph_coloring
cargo run -p rsi-ralph-examples --example hive_goal
```

## What each run shows

- **Composite scoring with an uncalibrated judge.** `math_problem` and
  `upgrade_file` blend a measured metric with an `LlmJudge`. The judge has no
  `JudgeCalibration`, so it can move the blended score but can never accept a
  candidate on its own. Every acceptance is backed by the metric. The
  supervisor's bad proposals are rejected in the trace: Wallis's slower
  product in one, and a regex rewrite that breaks Unicode slugs in the other.
- **Behavior as a hard gate.** `check_standard.py` runs behavior tests before
  it counts violations. A candidate that changes behavior fails its seeds and
  is rejected, however tidy it looks.
- **Hives.** `hive_goal` runs the hive path in `rsi-ralph-host`: an observed
  dossier, a typed context patch, paired evaluation on frozen private suites,
  and human-style approval.
  Its limits are deliberate. The hive has one seat on one desk, because the
  controlled runner supports single-seat hives. Proposals come from direct
  generation, which predicts `normalized_h`, rather than `RetreatEvolver`,
  whose `verified_quality` predictions `run_proposal_h` marks untestable. The
  accepted hive is deployed into a fresh coordinator, as `complete/controlled`
  does, so the run stops at the first acceptance.
- **Penalties.** In `hive_goal`'s first epoch a context edit sends the agents
  to a calculator tool the hive lacks. The incumbent completes its private
  runs and the candidate overspends on all of them, so it is rejected `HARMFUL`
  (the trace's `measured` row shows the failed seeds) and its reworded copy
  is stopped `COOLDOWN` without being evaluated.

## Benchmarking the loop

`benchmarks` runs the four script-loop subjects (`curve_fit`,
`graph_coloring`, `math_problem`, `upgrade_file`) as a matrix of benchmark ×
supervisor model × repeat and writes one comparison table: goal rate, median
epochs to goal, median tokens, median final metric, mean repetition share and
median wall time.

```sh
cargo run -p rsi-ralph-examples --example benchmarks -- --repeats 2
TINYRSI_LIVE_OPENROUTER=1 OPENROUTER_API_KEY=... \
  cargo run -p rsi-ralph-examples --example benchmarks -- \
  --models anthropic/claude-sonnet-4.5,openai/gpt-4.1-mini --repeats 5 --jobs 2
```

| Flag | Default |
| --- | --- |
| `--benchmarks a,b` | all four |
| `--models m1,m2` | `OPENROUTER_MODEL`, else `openai/gpt-4.1-mini` (live only) |
| `--repeats N` | 1 |
| `--jobs N` | 1 (runs executed concurrently) |
| `--out DIR` | `target/rsi-benchmarks/<unix-seconds>/` |

Offline, every run uses the benchmark's canned supervisor under the model
label `offline`, `--models` is ignored, and the command fails if any run
misses its goal. Each run writes `<out>/<benchmark>/<model>/<repeat>/`
(`events.jsonl`, `ledger.json`); `<out>/summary.json` and `<out>/summary.md`
hold the per-run records and the table. Pass
`--out docs/rsi/results/<date>/benchmarks` to keep a live result. The
repetition share and every column are defined in the
[loop benchmarks spec](../docs/specs/loop-benchmarks.md).

## Watching a run

Every run writes `target/rsi-examples/<example>/<run>/events.jsonl`. Point the
visualizer at that root:

```sh
cd apps/rsi-viz && npm ci && RSI_RUN_ROOT=../../target/rsi-examples npm run dev
```

## Going live

Set `TINYRSI_LIVE_OPENROUTER=1` and `OPENROUTER_API_KEY` (optionally
`OPENROUTER_MODEL`), and the script examples use a real model as both
supervisor and judge. Proposed code then runs locally with your privileges,
limited only by a copied workspace, a cleared environment, a timeout and an
output cap. Read the safety note in the
[script loop guide](../docs/guides/script-loop.md) first.

## Layout

- `src/lib.rs`: shared plumbing: the offline `Replies` and `Grader` model
  stand-ins, run directories, the Python interpreter lookup, and
  `live_model(id)`.
- `src/benchmarks/`: each script-loop subject (fixtures, brief, scorer,
  policy, canned replies) defined once, for the examples and the runner. The
  curve-fit fixtures are embedded from `crates/rsi-ralph/examples/script_loop/`.
- `src/harness/`: the runner's command line, matrix execution, per-run
  records, repetition share, aggregation and table.
- `<example>/main.rs`: the example. Python fixtures sit beside it: a fixed
  checker, a starting file, and `proposals/` for the canned rewrites.
