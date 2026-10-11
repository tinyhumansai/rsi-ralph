# Script loop: first live run (curve fitting)

One live run of the [script loop](../specs/script-loop.md) on the
`examples/script_loop` curve-fitting subject, with an LLM supervisor. The
question was whether the loop moves toward its goal on its own: observe,
ideate, execute, measure, score, improve, and repeat.

**Short answer:** it improved the script for real. Held-out RMSE fell 60%,
from 0.644 to 0.258, across three paired-evidence promotions. It then
stalled at about 4× the target (0.06) and stopped on its patience rule. The
loop machinery behaved as specified. Progress was limited by what the
supervisor can observe, not by the execute or score stages.

Raw evidence is in [`results/2026-10-11/script-loop/`](results/2026-10-11/script-loop/):
`events.jsonl` (the [loop trace](../specs/loop-trace.md)), `ledger.json`
(every candidate with per-seed metrics; local paths replaced by `<run-dir>`)
and `final-model.py` (the last incumbent).

## Setup

| Setting | Value |
| --- | --- |
| Subject | `model.py` (editable); `curve_fit.py` harness fixed |
| Signal | `1.5·e^(−0.25t)·sin(2.2t+0.4)+0.3` on [0,10], σ=0.05 noise, random 80/20 split |
| Metric | held-out `rmse`, minimize, target 0.06 (noise floor ≈ 0.05) |
| Seeds | 1, 2, 3 (paired; same seeds for incumbent and candidate) |
| Supervisor | OpenRouter `openai/gpt-4.1-mini`, one forced tool call per proposal |
| Policy | ≤ 8 epochs × 3 proposals, patience 3, 400k-token budget |
| Command | `TINYRSI_LIVE_OPENROUTER=1 cargo run -p tinyrsi --example script_loop -- --epochs 8 --proposals 3` |

## Trajectory

| Epoch | Incumbent RMSE | What was proposed | Outcome |
| --- | --- | --- | --- |
| 0 | 0.644 | baseline: line fit by 20 gradient steps | — |
| 1 | **0.458** | cubic least squares; two quadratic fits | cubic accepted; both quadratics failed every seed with a `SyntaxError` |
| 2 | **0.413** | cubic again (identical metric), degree 4, degree 5 | degree 4 and 5 accepted; best (degree 5) promoted |
| 3 | 0.413 | degree 7, degree 4, degree 3 | degree 7 inconclusive (CI −0.053…+0.057); others rejected |
| 4 | **0.258** | degree 6, degree 7 again, degree 8 | degree 8 accepted (+0.156, CI 0.092…0.219); repeat of degree 7 caught as a duplicate without running |
| 5–7 | 0.258 | Legendre/Chebyshev bases at degree 4–6 | all rejected; two Chebyshev variants blew up numerically (RMSE 87 and 203) |

The run stopped as `stalled` after epoch 7. Totals: 21 proposals, 4
accepted, 15 rejected, 1 inconclusive, 1 duplicate, 3 promotions. The
supervisor used 44,558 tokens.

## What worked

- **Paired scoring caught real and fake gains.** Every promotion had a
  confidence interval clear of zero. Changes that were the same model in
  another form (cubic rewritten, Legendre or Chebyshev bases spanning the same
  polynomials) produced identical per-seed metrics and were rejected rather
  than counted as progress.
- **Failures fed back into the loop.** Broken candidates were rejected with
  their stderr tail in the next prompt. In epoch 2 the supervisor's rationale
  explicitly fixes "the previous attempt [that] was syntactically incorrect".
- **Content-id dedupe saved a run.** The repeated degree-7 proposal was
  recognised as an existing candidate and not executed again.
- **The fixed harness held.** No proposal could touch the data or the scoring.

## Why it stalled

1. **The supervisor is blind to the data.** The prompt contains the brief,
   the metric history, rejected proposals and the incumbent `model.py`. It
   does not contain the harness, a plot, residuals or even the signal's
   shape. The brief only says "a noisy 1-D signal", so the supervisor kept
   searching polynomial degree. A damped sinusoid is not a polynomial; a
   model of the right family reaches the target in one step (the offline
   canned run does exactly that, at RMSE 0.047). The *observe* stage is the
   bottleneck: it records scalars, not diagnostics.
2. **No search memory beyond recent rejections.** Epochs 5–7 kept proposing
   orthogonal bases at degrees that had already been measured. These fit the
   same function space, so they cannot change the fit. The prompt lists recent
   rejections but no summary of which hypotheses have already been ruled out.
3. **Three seeds give wide intervals.** The degree-7 result was inconclusive
   with a ±0.055 interval. The proposal cannot win without more seeds or a
   sequential test.
4. **The proposer is mid-tier and runs at temperature 0.** Proposals within
   an epoch were often near-duplicates ("degree 6 / 7 / 8"), so three
   proposals per epoch bought little diversity.

## Next improvements, ranked

1. **Richer observation.** Let the subject print diagnostic lines (e.g.
   `DIAG residuals=…`, or a coarse residual-vs-t summary) and pass the last
   incumbent's diagnostics into the prompt. Optionally allow read-only
   context files (the harness) in `ScriptSubject`. This is the single change
   most likely to reach the target.
2. **Hypothesis ledger in the prompt.** Summarize tried model families and
   their best metric, so the supervisor stops resampling ruled-out space.
3. **Diversity across proposals.** Use a different seed or temperature per
   proposal slot. Or ask for K distinct families in one call, using the
   critic-pair retreat engine the hive loop already has.
4. **Adaptive seeds.** Add seeds when a result is inconclusive before
   rejecting, instead of a fixed three.

These are measurable with the same harness: rerun with the change and compare
epochs-to-target and tokens-to-target against this run.
