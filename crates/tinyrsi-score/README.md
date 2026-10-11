# tinyrsi-score

Decides whether a candidate is closer to the goal than the incumbent.

Goals in an improvement loop are often vague: "explain the proof clearly",
"bring this file up to standard". There is rarely a loss function, so this
crate separates two jobs:

- A **scorer** turns a subject into evidence: one quality in `[0, 1]` per seed
  (an `Assessment`).
- **`decide`** turns two assessments, the incumbent's and the candidate's on
  the same seeds, into `Accept`, `Reject` or `Inconclusive` with an interval.

Scorers never decide, and `decide` is the only decision rule.

## Scorers

| Scorer | Subject | Evidence | Accept-eligible |
| --- | --- | --- | --- |
| `MetricScorer` | per-seed metric values | `q = 1/2 + atan(s·m)/π` | yes |
| `ScriptedScorer` | anything | a closure (deterministic checks, fixtures) | the closure decides |
| `LlmJudge` | text | median of N forced `grade` calls (0–10) over a `ModelPort` | only with a passing `JudgeCalibration` |
| `BackroomsJudge` | text | mean of N tinybackrooms raters (1–7 goal fit, optional × novelty) over a `Conversant` | only with a passing `JudgeCalibration` |
| `Composite` | anything | weighted mean of members | its eligible members, renormalized |
| `Projected` | a richer subject | a scorer of one view of it | as the inner scorer |

`tinyrsi-verify` adds `VerifierScorer` for executed checkers and Lean proofs.

## The decision rule

1. `compare` decides on the qualities: a paired lower bound above the margin
   accepts, an upper bound at or below it rejects, and anything else is
   inconclusive.
2. **Eligibility.** An `Accept` stands only if `compare` also accepts on both
   sides' eligible qualities. Without eligible evidence the result is capped at
   `Inconclusive`. An uncalibrated judge can therefore move a composite's
   score, but the improvement must still show in calibrated or measured
   evidence alone.
3. **Units.** When both sides carry `MetricEvidence`, the reported interval is
   in metric units, exactly as the script loop reports it today. Otherwise it
   is in quality units.

## Also here

- `compare` / `ensure_independent` (`evaluation/`): paired comparison and
  independence of selection and confirmation draws.
- `score` / `compare_h` / `AnchorLedger` (`scoring/`): the normalized H score
  used by the hive funnel.
- `run_experiment` (`experiment/`): the preregistered original-versus-evolved
  protocol.

## Constraints

- Synchronous. Judges drive their backend futures on the calling thread with
  `tinyrsi_model::block_on`, so a backend must not need a specific async
  runtime.
- No process execution and no coordinator: running a candidate belongs to the
  loop or to `tinyrsi-verify`.
