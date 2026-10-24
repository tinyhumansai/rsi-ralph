# Script loop example: curve fitting

`curve_fit.py` is a fixed harness. It generates a noisy damped sinusoid from
`TINYRSI_SEED`, trains `model.fit` on 80% of the points, and prints the
held-out RMSE as `METRIC rmse=<value>`. `model.py` is the only editable file
and starts deliberately weak: a straight line after 20 tiny gradient steps
(RMSE about 0.64). The noise floor is about 0.05; the default target is 0.06.
Splitting harness and model keeps a proposal from changing the data or the
evaluation.

```sh
cargo run -p rsi-ralph --example script_loop
```

The supervisor's brief is `brief.txt`. The `curve_fit` benchmark in
`rsi-ralph-examples` (`cargo run -p rsi-ralph-examples --example benchmarks`)
embeds these same fixtures and brief.

## Offline (default)

A canned supervisor replays four proposals, two per epoch:

1. a cubic least-squares fit (better: accepted, promoted);
2. a constant predictor (worse: rejected);
3. an edit to `../curve_fit.py` (path escape: recorded as invalid);
4. a damped-sinusoid fit (accepted; reaches the target).

No network and no credentials are used, and token use is reported as 0.

## Live

```sh
TINYRSI_LIVE_OPENROUTER=1 OPENROUTER_API_KEY=... \
  OPENROUTER_MODEL=openai/gpt-4.1-mini \
  cargo run -p rsi-ralph --example script_loop -- --epochs 8 --proposals 3
```

**The model writes Python that runs on your machine.** It runs on a copy of the
workspace with a cleared environment, a 60 s timeout per seed and a 64 KiB
output cap, but with no network or filesystem sandbox. Use a container or VM if
that matters to you.

## Settings

| Flag | Environment | Default |
| --- | --- | --- |
| `--epochs N` | `TINYRSI_SCRIPT_LOOP_EPOCHS` | 4 |
| `--proposals K` | `TINYRSI_SCRIPT_LOOP_PROPOSALS` | 2 |
| `--target X` | `TINYRSI_SCRIPT_LOOP_TARGET` | 0.06 |
| `--budget T` | `TINYRSI_SCRIPT_LOOP_BUDGET` | 400000 supervisor tokens |
| | `TINYRSI_SCRIPT_LOOP_DIR` | `target/rsi-script-loop` |
| | `TINYRSI_PYTHON` | the interpreter behind `python3` |

The child environment is cleared, so `python3` is resolved once at startup to
its real executable (`sys.executable`), which also bypasses pyenv shims.

## Output

Each run writes `target/rsi-script-loop/<run-id>/`: `events.jsonl` (the
[loop trace](../../../../docs/specs/loop-trace.md)), `ledger.json`,
`candidates/<content-id>/` and the starting `workspace/`.
