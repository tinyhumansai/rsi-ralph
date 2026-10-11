# Pointing the script loop at your own script

The script loop improves any program that prints a number. This guide covers
what your script must do, how to describe it to the loop, and how to read the
results. The behavior contract is [`specs/script-loop.md`](../specs/script-loop.md);
the bundled example is [`examples/script_loop`](../../crates/tinyrsi/examples/script_loop/README.md).

## 1. Make the script measurable

Your command must:

- print one stdout line `METRIC <name>=<value>` per run, where `<value>` is a
  finite float (if it prints several, the last one counts);
- read its seed from the `TINYRSI_SEED` environment variable and be
  deterministic for a given seed — the loop compares candidate and incumbent
  **on the same seeds**, so noise from anything else hides real gains;
- exit with status 0, finish within the timeout and stay under the output cap;
- work with a cleared environment: only `PATH=/usr/bin:/bin`, `TMPDIR`,
  `TINYRSI_SEED` and the variables you list in `ScriptSubject::env` exist.
  Use an absolute interpreter path in `ScriptCommand::program` (for example
  the result of `python3 -c 'import sys; print(sys.executable)'`).

## 2. Separate what may change from what measures

List in `editable` only the files the supervisor may rewrite, as
workspace-relative paths. Keep data generation, train/test splits and the
metric computation in files that are **not** editable; otherwise the cheapest
"improvement" is to change the evaluation. The example splits a fixed
`curve_fit.py` harness from an editable `model.py` for this reason.

Editable files must exist and be UTF-8. Proposals replace whole files; any
other path (absolute, containing `..`, or unlisted) is refused and recorded as
an invalid proposal.

## 3. Describe the subject

```rust,ignore
let subject = ScriptSubject {
    name: "solver.py".into(),
    brief: "Make solve() faster without changing its answers. Standard library only.".into(),
    workspace: "path/to/project".into(),
    editable: vec!["solver.py".into()],
    command: ScriptCommand { program: "/usr/bin/python3".into(), args: vec!["bench.py".into()] },
    metric: MetricSpec { name: "seconds".into(), direction: MetricDirection::Minimize, target: Some(0.5) },
    seeds: vec![1, 2, 3, 4],
    timeout_ms: 60_000,
    output_cap: 65_536,
    env: vec![],
};
```

- `brief` is shown to the supervisor verbatim: state the goal and the rules.
- Use at least two seeds (required); more seeds tighten the paired interval.
- `target` is optional; without it the loop stops on epochs, patience or budget.

## 4. Choose a policy and a supervisor

`LoopPolicy::default()` runs up to 5 epochs of 2 proposals, stops after 2
epochs without a promotion, and accepts a candidate when the paired lower bound
of its improvement is above zero (`eval.margin = 0`). Raise `eval.margin` (in
quality units, see the module README) to demand larger gains.

`ScriptSupervisor::new(Arc::new(OpenRouterModel::new(model)?), SupervisorLimits::default())`
uses `OpenRouter`; any `ModelPort` works if its futures do not need a specific
async runtime. `token_budget` caps measured supervisor tokens across the run.

## 5. Run and read the results

```rust,ignore
let run = RunLocation { id: "solver-1".into(), dir: "target/rsi-script-loop/solver-1".into() };
let mut trace = JsonlTrace::create(run.dir.join("events.jsonl"), MonotonicClock::new())?;
let report = run_script_loop(&subject, &supervisor, &LoopPolicy::default(), &run, &mut trace)?;
```

- `events.jsonl` streams each stage ([schema](../specs/loop-trace.md)); tail it
  or open it in the visualizer.
- `ledger.json` lists every candidate with its parent, proposal, per-seed
  metrics and stderr tail, decision and interval, plus the incumbent chain.
- `candidates/<id>/` holds each candidate's full working copy; the final
  incumbent's files are your improved script.

## Safety

Every proposal is code that runs on your machine with your privileges. The loop
only copies the workspace, clears the environment, and enforces a timeout and
an output cap. It does **not** block network access or writes outside the
candidate directory. Run live supervision inside a container or VM, and never
with credentials in the subject's `env`.

## Scoring beyond the metric

`run_script_loop` decides on the metric alone. When the goal is partly about
the code itself, such as readability or following a standard, call
`run_script_loop_with` with a `tinyrsi_score::Scorer<ScriptCandidate>`, for
example a `Composite` of the metric (`MetricScorer` through a `Projected`) and
an `LlmJudge` reading the sources. An `LlmJudge` without a passing
`JudgeCalibration` can widen a margin but can never promote a candidate on its
own. See [`crates/tinyrsi-score/README.md`](../../crates/tinyrsi-score/README.md).
