# Scoring and supervised proposals

Related: [#2](https://github.com/tinyhumansai/tinyrsi/issues/2).

## Proposal funnel

Schema → formalize → guard → deduplicate → falsifiable prediction → grounded
evidence → prior ranking → replay screen → full selection → fresh confirmation.
Record every stopped item with its reason. A typed patch retains class,
mechanism, risk, effort and a predicted minimum improvement on named public
failure nodes. Originality is a diversity feature, never acceptance evidence.

The current deterministic funnel supplies guard, exact deduplication, grounded
references, paired screening/selection/confirmation and human approval.
Near-duplicate embeddings, blinded pairwise prior ranking, Bradley–Terry fitting
and alpha-spending sequential tests require further experimental infrastructure.

## Eval-owned quality

For goal result g, checkpoint passes p and fixed strength weights l:

`q = alpha*g + (1-alpha)*sum(l*p)/sum(l)`

Proof weighs 1.0, execution 0.9, schema and private reference 0.7.
A rubric judge weighs its measured agreement with human labels; preference
weighs half that agreement. `JudgeCalibration` estimates binary Cohen kappa,
requires informative label variation, and discounts labels below the frozen
calibration threshold. The host owns the calibration dataset.

Out-of-budget completion earns no goal credit. Budgets are enforced during
execution; a post-hoc token count alone cannot prevent overspending.
Self-created blueprint nodes cannot earn eval checkpoint credit.

## Normalization and H

For every family f, freeze a baseline b, a reference r and weight w, with r>b.
Compute `H_f=(mean(q_f)-b)/(r-b)`. Normalize before aggregating so easier task
families do not dominate through their raw scale or number of tasks.

`H=IQM(w_f*H_f) - mu*FalseClaimRate - nu*TamperRate
   - lambda*max(0,Cost(candidate)/Cost(incumbent)-1)`

The interquartile mean trims a quarter of mass at each tail, including fractional
weight when there are fewer than four families. Report the score with a
bootstrap interval: resample tasks within family and seed clusters, using
identical draws for paired arms. Pin bootstrap seed and policy for replay.

Hard gates: no family regresses beyond its frozen limit, token cost respects
tolerance, false-claim rate and tamper rate do not worsen. A positive score that
breaks a hard gate is rejected. Accept only when the paired lower delta bound
exceeds the margin, then require confirmation and human approval.

Private confirmation uses fresh task identities and seeds; it is not a
second pass over the selection winners. Anchor tasks never feed proposal
generation or selection. Only frozen anchor scores draw the progress curve.
Sustained anchor decline or widening selection–anchor gaps ask the host to
roll back and freeze the responsible class. Contamination halts evaluation.

## What results establish

Raw `compare` reports a paired mean and ±2 standard errors across seed-level
means. This is a noise heuristic, especially with very few seeds, rather than
a calibrated confidence claim. The normalized bootstrap API also needs diverse
curated tasks and enough independent samples for useful coverage. A two-seed
arithmetic smoke run establishes connectivity and state-machine behavior only.
Judge calibration, contamination checks over actual store contents, held-out
benchmark ownership and repeated ignition trials are host obligations.
