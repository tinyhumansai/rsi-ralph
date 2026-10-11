# Recorded scoring example

This is issue [#2](https://github.com/tinyhumansai/tinyrsi/issues/2)'s worked
example, using the completed post-review OpenRouter run from merged
[PR #10](https://github.com/tinyhumansai/tinyrsi/pull/10).
No new provider run, inferred proposal labels or synthetic task results are used.
The [scoring specification](specs/scoring.md) defines the intended system;
the [acceptance map](scoring-acceptance.md) identifies implementation limits.

## One real retreat through the funnel

The [worked receipt](results/2026-10-11/post-review/worked-report.json) records
50 typed proposal occurrences and 30 invalid formatter occurrences. This exceeds
the required twenty proposals. The actual tinybackrooms Engine made 121 calls,
using 158,098 measured generation tokens; its 1,620,641-token reservation was
below the 3,000,000 cap. Reservations are not provider billing totals.

| Terminal outcome in current API | Typed occurrences |
| --- | ---: |
| `WRONG_CLASS` | 17 |
| `UNGROUNDED` | 31 |
| `GUARD` | 1 |
| `NO_GAIN` after normalized screen | 1 |
| **Total** | **50** |

Invalid formatting remains a separate count, not invented typed proposals.
All fifty typed occurrences have individual outcomes in the receipt; the invalid
counter does not supply thirty full per-item `ProposalRecord` rows. The runner's
existence-based grounding and coarse reasons are the implemented gates, not all
the semantic checks or detailed codes in the intended specification.

The one screened candidate is
`sha256:d751850a0cfde35c607d4b5f661d528a2c782d2f055bb92516a211461cd64ecf`.
The recorded screen gives these normalized-H estimates and two-sided bootstrap
intervals:

| Measure | Incumbent | Candidate |
| --- | ---: | ---: |
| Code family | 0 | 0 |
| Proof family | 0 | 0 |
| Reference family | 1 | 1 |
| Harness/schema family | 1 | 1 |
| False-claim rate | 0.5 | 0.5 |
| Tamper rate | 0 | 0 |
| Cost ratio | 1 | 1.0148423005565863 |
| H | 0.45 | 0.4498515769944341 |
| H interval | [0.45, 0.45] | [0.4498515769944341, 0.4498515769944341] |

The frozen toy anchors are baseline zero, reference one and importance one in
each family. IQM of `[0,0,1,1]` is 0.5. With false-claim penalty 0.1 and cost
penalty 0.01:

```text
H(incumbent) = 0.5 - 0.1*0.5                         = 0.45
H(candidate) = 0.5 - 0.1*0.5 - 0.01*(1.0148423 - 1) = 0.449851577
paired delta = -0.00014842300556588972
delta interval = [-0.00014842300556588972, -0.00014842300556588972]
```

The upper delta bound is below zero, so the actual decision is **reject / NO_GAIN**.
No candidate reached selection, confirmation or approval. The deployed baseline
survived durable restart. Discovery ranks did not override the rejection.

## Separate confirmation and deployment check

The [controlled receipt](results/2026-10-11/post-review/controlled-report.json)
comes from an independently bounded, narrower proposal schema. It is not a
survivor from the retreat above and cannot replace any rejected proposal or
failed experimental assignment. The model-authored proposal used 645 measured
generation tokens.

| Stage | Incumbent H and interval | Candidate H and interval | Paired delta and interval |
| --- | --- | --- | --- |
| Screen | 0.45 [0.45, 0.45] | 0.975 [0.975, 0.975] | 0.525 [0.525, 0.525] |
| Selection | 0.45 [0.45, 0.45] | 0.975 [0.975, 0.975] | 0.525 [0.525, 0.525] |
| Fresh confirmation | 0.45 [0.45, 0.45] | 0.975 [0.975, 0.975] | 0.525 [0.525, 0.525] |

Values are rounded here; JSON retains full floating-point precision. Code rose
from zero to one; reference/schema stayed one, proof stayed zero. Candidate IQM
is one, false-claim penalty is `0.1*0.25`, and its cost ratio is below one, giving
H 0.975. No family regressed. Zero proof credit is not a proof success.

The denied approval gate retained the baseline. A separate sandbox-authorized
archive accepted the same exact proposal, deployed it through SQLite, verified
a real deployed turn, rolled back to the exact parent/memory watermarks, and
reopened successfully. This validates those ports, not a production human-review
service or a learned proposal ranker.

## What the evidence supports

These are deliberately impaired public toy fixtures with theoretical anchors,
repeated code/schema semantics and degenerate intervals. The intervals do not
establish generalization or calibrated error control. The separate
[24-attempt comparison](results/2026-10-11/post-review/experiment-report.json)
retained 20 rejections and four failures and was **Inconclusive**; it found no
accepted anchor gain. Its Original/Evolved labels denote human-authored frozen
configurations, not an accepted learned orchestrator.

Earlier v7 receipts remain separate. Full tool versions, protocol identities and
execution limits are in the [integration report](complete-live.md). This worked
example satisfies the recorded-data requirement without claiming every intended
ranking, monitoring or cleanup mechanism has been implemented.
