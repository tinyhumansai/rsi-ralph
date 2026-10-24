# Benchmark harness for the script loop

Status: Implemented. Plan: [`../plans/loop-benchmarks.md`](../plans/loop-benchmarks.md).
Roadmap item 6 of the [script loop roadmap](../rsi/script-loop-roadmap.md).

## Problem

Every claim about the script loop so far rests on one run per configuration
([diagnostics results](../rsi/diagnostics-results.md)). A run's outcome
depends on the supervisor's sampling and on how well it implements an idea,
so one run cannot tell a better loop from a lucky one. The roadmap's items 1
to 5 each name a measure (repetition, epochs to goal, tokens per accepted
candidate), and none of them can be compared fairly without repeated runs
summarized the same way every time.

The benchmark subjects already exist, but each lives inside its own example
binary, so nothing can run them in a matrix.

## Goals

1. Run every selected benchmark × supervisor model × `N` repeats, each run
   independent, and write one comparison table.
2. Reuse the existing example subjects: one definition of each benchmark's
   fixtures, brief, scorer, policy and canned supervisor, called by both the
   example binaries and the runner.
3. Run offline by default, with each benchmark's canned supervisor, so CI can
   execute the runner in seconds.
4. Keep the measurement logic (per-run record, repetition share, aggregation,
   Markdown rendering) pure and unit-tested.

## Non-goals

- Changing the loop, its prompt, the proposal schema or the ledger. The
  hypothesis ledger (roadmap item 1) adds an `approach` label in a separate
  change; this harness only reads what the ledger already records.
- Statistical tests between configurations. The table reports rates and
  medians; deciding significance is left to the reader for now.
- Cost in currency. Tokens are reported as measured; prices vary by provider.
- The hive example (`hive_goal`), which is not a script loop.

## Benchmarks

| Name | Subject | Metric | Target | Policy (epochs × proposals, patience) | Scorer |
| --- | --- | --- | --- | --- | --- |
| `curve_fit` | `model.py` behind `curve_fit.py` | `rmse` ↓ | 0.06 | 4 × 2, 3 | metric |
| `graph_coloring` | `coloring.py` behind `check_coloring.py` | `colors` ↓ | 6 | 6 × 2, 3 | metric |
| `math_problem` | `pi.py` behind `check_pi.py` | `digits` ↑ | 12 | 4 × 1, 3 | 70% metric + 30% judge |
| `upgrade_file` | `slug.py` behind `check_standard.py` | `violations` ↓ | 0 | 4 × 1, 3 | 60% metric + 40% judge |

Each is the existing example, unchanged. `rsi_ralph_examples::benchmarks`
holds them as a `Benchmark` enum; the Python fixtures stay where they are and
are embedded with `include_str!`. The curve-fit fixtures stay in
`crates/rsi-ralph/examples/script_loop/`, which keeps its own self-contained
example; the shared brief moves to a `brief.txt` beside them so both read one
copy.

`Benchmark::run(run, python, models)` writes the fixtures to
`run.dir/workspace/`, builds the subject, the supervisor and the scorer, and
runs `run_script_loop_with` with a `JsonlTrace` at `run.dir/events.jsonl`. A
benchmark with a judge needs a judge port; omitting it is an error
(`judge required`).

## Command line

```sh
cargo run -p rsi-ralph-examples --example benchmarks -- \
  [--benchmarks a,b] [--models m1,m2] [--repeats N] [--jobs N] [--out DIR]
```

| Flag | Default | Meaning |
| --- | --- | --- |
| `--benchmarks` | all four | comma-separated benchmark names |
| `--models` | `OPENROUTER_MODEL`, else `openai/gpt-4.1-mini` | comma-separated `OpenRouter` model ids (live only) |
| `--repeats` | 1 | runs per benchmark × model, at least 1 |
| `--jobs` | 1 | runs executed concurrently, at least 1 |
| `--out` | `target/rsi-benchmarks/<unix-seconds>` | output root |

Unknown flags, unknown benchmarks, a missing value, an empty list or a zero
count are errors, reported before any run starts.

**Offline** (`TINYRSI_LIVE_OPENROUTER` unset): every run uses the
benchmark's canned supervisor and scripted judge, under the single model
label `offline`. `--models` is ignored with a printed note.

**Live** (`TINYRSI_LIVE_OPENROUTER=1`, `OPENROUTER_API_KEY` set): each run
builds a fresh `OpenRouterModel` from the model id
(`rsi_ralph_examples::live_model`), used as both supervisor and judge. The
environment is never mutated to select a model.

Runs are independent: each gets fresh model ports, a fresh canned queue and
its own directory. With `--jobs N` they run on `N` scoped threads pulling from
one queue; the summary is ordered by benchmark, model and repeat regardless.

## Output

```text
<out>/
├── summary.json
├── summary.md
└── <benchmark>/<model-label>/<repeat>/   # repeat counts from 1
    ├── events.jsonl
    ├── ledger.json
    ├── candidates/ …
    └── workspace/
```

`<model-label>` is the model id with every character outside
`[A-Za-z0-9._-]` replaced by `_` (`anthropic/claude-sonnet-4.5` →
`anthropic_claude-sonnet-4.5`). The run id is
`<benchmark>-<model-label>-<repeat>`.

### Per-run record

Extracted from the run's `ledger.json`, plus wall time measured by the
runner. A run that returns an error is still recorded: `stop` is `error`,
`error` holds the message, and every count comes from the ledger if one was
written (otherwise zero and no metrics).

| Field | Source |
| --- | --- |
| `benchmark`, `model`, `repeat` | the matrix cell |
| `stop` | `ledger.stop`, `error` when absent |
| `goal_reached` | `stop == goal_reached` |
| `epochs` | highest `epoch` among ledger rows (equals `LoopReport::epochs`) |
| `metric` | `ledger.metric.name` |
| `baseline_metric`, `final_metric` | first and last `incumbents` entry; `null` without one |
| `promotions` | `incumbents.len() - 1` |
| `proposals` | rows with `epoch ≥ 1` |
| `rejected` | proposals whose decision is not `accept` (as `LoopReport::rejected`; includes duplicates and invalid ones) |
| `duplicates`, `invalid` | proposals with decision `duplicate`, `invalid` |
| `repeats`, `repetition_share` | see below |
| `tokens` | `ledger.tokens` |
| `wall_ms` | wall time of the run, including fixture setup |
| `run_dir` | the run's directory |
| `error` | the run's error message, `null` on success |

### Repetition share

A conservative proxy for "the supervisor resubmitted an idea already ruled
out in this run". Take the proposals (rows with `epoch ≥ 1`) in ledger order.

- A **ruled-out source** is a proposal that was not promoted and whose
  decision is `reject`, `inconclusive`, `invalid`, `error`, or `accept`
  (accepted but beaten by a better candidate in its epoch). Duplicates are
  never sources (they echo an earlier row), and neither is anything promoted.
- The **normalized hypothesis** is the hypothesis lowercased, with every run
  of non-alphanumeric characters replaced by one space, then trimmed.
- Proposal `i` is a **repeat** when some earlier source `j < i` has the same
  content id (both present), or the same non-empty normalized hypothesis.

`repetition_share = repeats / proposals`, and `null` when there are no
proposals. Resubmitting the incumbent is not counted, and neither is a
rewording that changes a word: the measure undercounts rather than guesses.
It is a proxy until the hypothesis ledger adds an `approach` label, after
which the same field can count repeated approaches instead.

### Aggregate rows

One row per benchmark × model, in matrix order:

| Field | Definition |
| --- | --- |
| `runs` | records in the cell |
| `goals` | records with `goal_reached` |
| `goal_rate` | `goals / runs` |
| `median_epochs_to_goal` | median `epochs` over goal runs; `null` with none |
| `median_tokens` | median `tokens` over all runs |
| `median_final_metric` | median over runs with a final metric; `null` with none |
| `mean_repetition_share` | mean over runs with a share; `null` with none |
| `median_wall_ms` | median `wall_ms` over all runs |

The median of an even count is the mean of the middle two. `summary.json` is
`{ "rows": [...], "runs": [...] }` with the fields above; a unit test pins
its shape. `summary.md` is a heading, the command's settings and a Markdown
table with columns Benchmark, Model, Goal (`k/N`), Epochs to goal, Tokens,
Final metric, Repetition and Wall (s), where `null` renders as `–`. The
runner prints the table on completion.

## Invariants

- Each existing example binary keeps its output and its offline assertion
  that the goal is reached.
- The runner never stops the matrix on one run's error; it records it.
- The runner exits non-zero offline when any run misses its goal, so CI
  notices a broken benchmark.
- No new dependency.

## Acceptance criteria

- `cargo run -p rsi-ralph-examples --example benchmarks -- --repeats 2` runs 8
  offline runs, prints a table with goal rate `2/2` for every benchmark, and
  writes `summary.json`, `summary.md` and one run directory per run.
- Unit tests cover argument parsing errors, record extraction, the
  repetition share (content repeat, hypothesis repeat, duplicate of the
  incumbent, empty run), aggregation, rendering and the `summary.json` shape.
- The four existing script examples still reach their goals offline.

## Open questions

None blocking. Results worth keeping are committed under
`docs/rsi/results/<date>/` by passing `--out`.
