# TinyRSI examples

Three end-to-end runs of the improvement loop. Each one proposes changes
(evolve), measures and judges them (score), and keeps only what is better
(the loop). Each writes its live trace for the visualizer. All three run
offline by default, with canned model replies and scripted judges, so they
need no network or credentials and run in CI.

| Example | Subject | Evolve | Score |
| --- | --- | --- | --- |
| [`math_problem`](math_problem/main.rs) | `pi.py`, which must compute π to 12 digits | script supervisor rewrites the method | correct digits (metric) + an LLM judge of the method's justification |
| [`upgrade_file`](upgrade_file/main.rs) | `slug.py`, which must meet a written coding standard | script supervisor rewrites the file | standard violations, with behavior tests as a gate (metric) + an LLM judge reading the source |
| [`hive_goal`](hive_goal/main.rs) | a tinyhivemind hive whose agents must solve private tasks | proposals of typed hive patches | paired private-suite evaluation (normalized H) + an approval gate |

```sh
cargo run -p tinyrsi-examples --example math_problem
cargo run -p tinyrsi-examples --example upgrade_file
cargo run -p tinyrsi-examples --example hive_goal
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
- **Hives.** `hive_goal` runs the hive path in `tinyrsi-host`: an observed
  dossier, a typed context patch, paired evaluation on frozen private suites,
  and human-style approval.
  Its limits are deliberate. The hive has one seat on one desk, because the
  controlled runner supports single-seat hives. Proposals come from direct
  generation, which predicts `normalized_h`, rather than `RetreatEvolver`,
  whose `verified_quality` predictions `run_proposal_h` marks untestable. The
  accepted hive is deployed into a fresh coordinator, as `complete/controlled`
  does, so the run stops at the first acceptance.

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
  stand-ins, run directories, and the Python interpreter lookup.
- `<example>/main.rs`: the example. Python fixtures sit beside it: a fixed
  checker, a starting file, and `proposals/` for the canned rewrites.
