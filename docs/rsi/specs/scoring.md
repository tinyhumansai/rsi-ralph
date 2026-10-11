# Scoring and supervised proposals

Related: [#2](https://github.com/tinyhumansai/tinyrsi/issues/2).

This specification defines the intended system, including mechanisms not yet
implemented. The [acceptance and test map](../scoring-acceptance.md) distinguishes
those mechanisms from today's APIs. The [recorded worked example](../scoring-worked-example.md)
uses merged live receipts rather than invented results.

## Ownership and measurement identity

The improvement object is `(hive package, pinned memory)`. The host owns checks,
budgets, scoring configuration, private human labels and approval. The
orchestrator may learn a proposal prior, but cannot edit its judge.
Selection is the training signal, confirmation is validation, and anchors are
the independent test signal. The objective is `-H`; staged classes and smaller
patches regularize the search. Pinned memory is learned state outside the package
and must remain part of the measured candidate identity.

| Partition | Purpose | Proposer-visible data |
| --- | --- | --- |
| Public dossier / replay | Target failures and canary regressions | Redacted public references and aggregate outcomes |
| Selection | Rank and compare candidates | Aggregate labels, never private tasks or answers |
| Confirmation | Confirm a winner on fresh tasks and seeds | Aggregate disposition, never tasks, answers or per-task feedback |
| Anchor | Independently measure incumbent progress | Aggregate progress curve; private data stays with the host |

Confirmation and anchor tasks never enter retreat prompts, prior examples or
selection. Anchor signals may trigger host intervention, but cannot select the
best proposal. Suite fingerprints bind actual private content, checks and seeds.
Every measurement envelope records package identity, complete memory watermarks,
suite purpose/fingerprint, policy/calibration version, tasks/seeds, caps, actual
usage and health denominators. Private rows stay in the host archive.
Changing references or policy begins a new measurement series.

## Proposal funnel

Schema → formalize → guard → deduplicate → falsifiable prediction → grounded
evidence → prior ranking → replay screen → full selection → fresh confirmation.
Record every stopped item with its reason. A typed patch retains class,
mechanism, risk, effort and a predicted minimum improvement on named public
failure nodes. Originality is a diversity feature, never acceptance evidence.

| Stage | Required operation | Rejection codes |
| --- | --- | --- |
| 0 Schema | Validate the complete typed proposal; account for invalid/repair occurrences | `SCHEMA` |
| 1 Formalize | Lower an atomic patch, resolve targets/preconditions and validate invariants | `UNFORMALIZABLE`, `INVALID_REF`, `INVARIANT` |
| 2 Guard | Exclude protected edits, unauthorized memory reach and held-out content/canaries | `GUARD`, `MEMORY_REACH`, `CONTAMINATION` |
| 3 Dedup | Check accepted/rejected canonical hashes and semantic near-duplicates | `DUP_ACCEPTED`, `DUP_REJECTED` |
| 4 Falsifiable | Require metric, direction, minimum effect and public failure targets | `UNTESTABLE` |
| 5 Grounded | Resolve references and verify that they support the claimed failure | `UNGROUNDED` |
| 6 Prior | Rank valid proposals without granting acceptance credit | None |
| 7 Replay | Paired targeted-failure execution and a frozen canary set | `NO_EFFECT`, `CANARY_REGRESSION` |
| 8 Selection | Full paired ΔH and hard constraints | `NO_GAIN`, `CONSTRAINT`, or inconclusive disposition |
| 9 Confirmation | Independent tasks/seeds, same caps and frozen policy | `NOT_REPLICATED` |

`WRONG_CLASS` additionally rejects proposals outside the epoch's authorized
class. These seventeen rejection codes form the intended closed taxonomy.
Acceptance, inconclusive evidence, approval denial and host failure are separate
dispositions. Current `ReasonCode` combines some of these reasons; the acceptance
map records the projection without claiming new wire variants already exist.

Every occurrence retains its attempt identity and terminal outcome. Missing
checks/providers are host failures, not zero-cost verified observations. Invalid
raw outputs remain private; retain a safe occurrence identity/reason. A later
schema repair or duplicate does not erase its earlier occurrence.

H predictions commit before evaluation to increasing `normalized_h`, a positive
`minimum_gain`, and public dossier targets. Direction is implicit in today's
`Prediction`. The effective margin is the maximum of host margin and declared
effect. Predictions cannot change after a screen result. Grounding must check
both reference existence and semantic support; today's runner checks existence.

Exact dedup freezes a canonical typed-patch digest, including semantic operation
order, identities and preconditions. Near-duplicate admission freezes the
embedding model, threshold and comparison corpus. A rejected near-duplicate
requires recorded new public evidence or changed preconditions; a new title is
not new evidence. Similarity is not a private quality label.

### Prior ranking and test allocation

A blinded panel compares proposal pairs against the same public dossier, hides
source/discovery rank, uses distinct model families and swaps positions. Retain
ties, abstentions and both orders. Freeze judge versions, rubric and pair
schedule. The rubric covers grounding, causal mechanism, deterministic reach
(class, identities, changed content), cost and reversibility.

Bradley–Terry produces a relative ranking, not an improvement probability.
Disconnected comparison groups cannot form a single identified ranking.
Calibrate ranking/features separately against earlier selection outcomes to
estimate `p_improve` and conditional positive gain. tinybackrooms comparative
and judge scores are prior features; originality only diversifies exploration.

`EV(p) = p_improve(p) * E[delta_h | improvement, p] / remaining_test_cost(p)`

Test cost must be positive and in frozen units. Keep estimate provenance and
uncertainty. Insufficient history means an explicitly uncalibrated prior and
uniform testing, not confidence inferred from rank. Reserve a predeclared
exploration share to make testing propensity and selection bias visible.

Successive halving gives many proposals a small replay allocation and survivors
more selection seeds. Freeze round sizes, survivor counts, total caps and
deterministic tie-breaking before outputs. Evaluate each patch against the same
incumbent with pinned memory. At equal gain, prefer smaller patches using frozen
reach/description-length features. A composition of accepted patches is a new
candidate and must be tested; separate improvements need not compose safely.

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
Require `alpha` in `[0,1]` and at least one positive-weight checkpoint. Partial
credit may describe checks completed before exhaustion; over-cap evidence cannot
enter a valid paired comparison. Failed/missing checks cannot become passes.

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

Require `0 <= baseline < reference <= 1`, positive family weights and a complete
task-by-seed matrix with at least two shared seeds. Weights multiply family
values before trimming; they do not duplicate easy tasks or families. Normalized
scores are not clamped: exceeding a reference or falling below a baseline is
meaningful. Nonfinite arithmetic invalidates the measurement.

The intended secondary diagnostic is the weighted geometric mean of `H_f`, using
exponents `w_f/sum(w)`. Zero produces zero; a negative family score makes that
diagnostic undefined and must be reported without clipping or an epsilon. IQM
remains the selection aggregate. This diagnostic is not implemented today.

Host telemetry defines false claims as failed claimed completions divided by
observed claims, and tamper rate as rejected protected-surface attempts divided
by relevant observed edit/tool attempts. Publish counts and versioned
denominators. No attempts means zero rate with zero denominator, not measured
safety. Retreat guard and solver-action populations remain separate. Cost uses
actual paired usage; reservations are a different ledger. Both arms zero cost
give ratio one; a positive candidate cost against a zero incumbent cannot yield
a valid finite relative ratio.

The implemented interval is a deterministic two-sided 95% percentile bootstrap,
with at least 200 draws. Resample task clusters within families and shared seed
clusters, preserving pairing and recomputing normalization/IQM. More bootstrap
draws do not create more observations. These intervals condition on supplied
health rates and references; broader inference needs a justified method for
their uncertainty. Equal endpoints on tiny deterministic tasks do not establish
benchmark generalization or calibrated sequential error control.

Hard gates: no family regresses beyond its frozen limit, token cost respects
tolerance, false-claim rate and tamper rate do not worsen. A positive score that
breaks a hard gate is rejected. Accept only when the paired lower delta bound
exceeds the margin, then require confirmation and human approval.

Private confirmation uses fresh task identities and seeds; it is not a
second pass over the selection winners. Anchor tasks never feed proposal
generation or selection. Only frozen anchor scores draw the progress curve.
Sustained anchor decline or widening selection–anchor gaps ask the host to
roll back and freeze the responsible class. Contamination halts evaluation.

## From evidence to action

Safety and contamination failures take priority over improvements. The acceptance
map distinguishes implemented ports/signals from future automatic controllers.

| Signal | Required action |
| --- | --- |
| Lower paired ΔH bound strictly exceeds effective margin; constraints hold | Confirm independently, obtain exact-candidate human approval, then accept |
| Upper ΔH bound below zero | Reject with `NO_GAIN` |
| Interval does not clear margin or rule out gain, including equality | Inconclusive: stop, or use a preregistered sequential procedure within remaining caps |
| H improves but any hard constraint fails | Reject with `CONSTRAINT`; retain the regression label |
| Winner fails confirmation or reuses tasks/seeds | `NOT_REPLICATED`; do not seek approval |
| Sustained anchor decline or widening selection–anchor gap | Roll back to an eligible best-anchor ancestor; freeze responsible class |
| Tamper rate exceeds frozen review threshold | Halt generation/deployment and request human review |
| Held-out canary found in hive memory | Halt, quarantine snapshots, forget offending events, invalidate affected anchor reports, require clean remeasurement |
| Class eligibility/acceptance near zero over frozen epoch window | Switch authorized class or broaden retreat mode, lens or variations |
| Prior calibration deteriorates | Reduce its EV influence; allocate more tests uniformly |
| Approval denied/unavailable or execution fails | Preserve incumbent; retain denial/host-failure disposition |

Hard gates include every family regression allowance, no worse false-claim or
tamper rates, and token cost within `(1+epsilon)` of the incumbent. A zero cost
tolerance uses exact integer totals. Malformed evidence is not an ordinary loser.
Freeze penalties, constraints and margins before generation, using host
calibration; live toy settings are not production defaults.

Anchor monitoring requires configured consecutive epochs; skipped epochs reset
streaks. `AnchorLedger` returns advice and the best point-estimate epoch, not
proof of an optimal ancestor. Before rollback the host checks ancestry, comparable
suite/policy/memory identities and recoverability. Selection and live deployment
remain distinct until activation succeeds. Halts must survive restart; a store
error or score API alone does not implement a halt controller.

Freeze tamper thresholds, class windows, class-attribution rules and prior-health
thresholds. Report confirmed eligibility separately from human-approved
deployment so review denials do not masquerade as poor proposal efficacy.

Fresh confirmation addresses selection of the best among many proposals; only
independent anchors draw progress. Sequential tests must precommit seed draws,
stopping looks, total spend and error allocation. For example,
`a_k = alpha/(k*(k+1))` sums to at most `alpha`, but every look still needs a valid
level-`a_k` procedure. Applying this schedule to uncalibrated bootstrap intervals
does not manufacture error control. Today inconclusive selection stops; an
inconclusive screen may proceed to the frozen full selection stage.

## Vague-domain scoring

Extract independently checkable claims first: citation resolution, reproducible
numbers, executable snippets and required sections. Use the strongest available
eval-owned verifier for each. A citation's existence does not verify its content.
Record unsupported claims explicitly.

For remaining subjective quality, compare blinded candidate/incumbent outputs
on the same task with distinct-model judges and swapped positions. Freeze
rubric levels with descriptions and worked exemplars. Bradley–Terry or Elo gives
relative ratings, not kernel evidence or a task-quality probability.

The host collects rotating private human labels, with stable instructions and
adjudication, then freezes calibration before that epoch's candidate scoring.
Track judge/rubric identity, position bias, sample size, missing ratings and human
disagreement. Twenty to fifty pairs is a starting collection target, not a
precision guarantee. Labels and private rubric answers never enter retreat
context or hive memory.

Binary Cohen kappa is `(observed_agreement-chance_agreement)/(1-chance_agreement)`.
Matched labels need informative variation; degenerate data is an error, not
perfect reliability. Below the frozen threshold the judge contributes zero;
otherwise rubric strength is `max(0,kappa)` and preference strength half that.
If every checkpoint loses positive strength, quality is invalid. Ordinal rubrics
require a separately specified calibration; Spearman calibration and the judge
panel are not implemented today.

## Memory and attribution

Pin every H measurement to a complete immutable memory image. Evaluation reads
it and writes only to throwaway namespaces. Spec edits hold memory fixed;
learning edits compare the incumbent spec with old versus new memory. Shared or
reassigned bindings credit the identity that authored the learning, not the seat
currently reading it. Forks preserve parent/version provenance. Rollback restores
exact watermarks rather than only a package ID.

Periodically compare memory-off under the same tasks and caps, reporting quality
and cost without attributing memory gains to spec learning. Track repeated
failures, recall usefulness/clipping, recall/remember latency, growth and canary
hits, with explicit denominators. General event-forgetting/telemetry integrations
remain host responsibilities; local snapshot isolation is not a CortexDB cleanup
capability.

## Outcome dataset and feedback

Each attempt's row contains public dossier/proposal features, source identity
(run, mode, lens, model), epoch/class, patch hash, original prediction, prior
version/estimates, last stage, reason/disposition and measured ΔH/interval/family
results when available. Host failures are missing measurements, never measured
no gain. The row extends language lineage; it cannot replace ancestry.

The intended `HiveScore` envelope adds memory identity, policy, suite purpose and
tasks/seeds to H. Today's `ScoreEstimate` does not carry all provenance itself;
archive events and candidate state supply it. The fuller `ProposalRecord` and
source/prior dataset are future wire-contract work, not new types in this change.

| Intended record | Required fields |
| --- | --- |
| `Prediction` | Metric, direction, positive minimum effect, public dossier/node targets |
| `ProposalRecord` | Occurrence ID, epoch, source run/mode/lens/model, patch hash/class, prediction, prior version/judges/probability/conditional gain, last stage, optional reject code and measured ΔH/CI/per-family outcomes, terminal disposition, optional lineage candidate ID |
| `HiveScore` | H/CI/per-family scores, health rates with counts, exact memory snapshot, policy version, suite purpose/fingerprint, private task IDs and seeds |
| `CalibrationRecord` | Judge/rubric/version, private label-batch identity, sample count, agreement/threshold, frozen epoch |

These are host-owned records, not proposer-supplied claims. Finalize versioned
field names and serde representations before shipping them. The complete
history retains every stopped occurrence; public exports omit private tasks,
answers, raw outputs and calibration labels.

Fit priors from earlier replay/selection labels, without confirmation or anchor
rows. Define the target event before fitting: `y=1` means the selection lower
bound cleared margin with all gates; clear losers/constraint failures give
`y=0`. Inconclusive and failed measurements are censored with counts retained.

- Brier: `mean((p_improve-y)^2)` on uncensored measured eligibility labels.
- Spearman: prior rank against realized selection ΔH, with ties/sample size.
- Precision@k: eligible outcomes in predeclared top k, with missing/censored
  outcomes reported separately rather than silently excluded.

These measure eligibility, not causal truth. Report label coverage and testing
propensity. Start with few-shot accepted/rejected examples; a trained lightweight
ranker is optional later work. Freeze refitted priors for the next epoch, not
the current one. Safe public outcomes feed labelled `prior_idea` / `move_away_from`
seeds and durable recall. Learned comparative criteria or scorer weights change
the proposer, never H or its verifiers. Compare each mode/lens/model's eligibility,
acceptance, cost and calibration against a budget-matched direct-proposer arm.
Recall is a bounded cache; the archive/journal preserves full history.

## Host configuration that must be frozen

Before generation, the evaluation owner records `alpha`, verifier/calibration
weights, empirical baseline/reference provenance, family importance/regression
limits, health denominators/penalties, cost tolerance, practical margin, sampling
and uncertainty settings, and monitoring thresholds/windows. Calibrate numerical
choices on a separate pilot dataset and measured seed variance; do not choose
them from the winning candidate. No universal production values are asserted.
The toy example's zero/one references are theoretical, not empirical calibration.

The host also names the private-suite curator and human-label owner, epoch caps,
authorized classes and permanent approval requirements. Rotating an anchor set
requires versioned re-baselining; do not splice incompatible scores into one
progress curve. If a required owner, check or calibration is unavailable, retain
a host-failure/uncalibrated disposition rather than silently supplying a model
oracle or weakening the frozen gate.

## What results establish

Raw `compare` reports a paired mean plus/minus its policy's standard-error
multiplier (at least two, default two), across seed-level means. This is a noise
heuristic, especially with very few seeds, rather than
a calibrated confidence claim. The normalized bootstrap API also needs diverse
curated tasks and enough independent samples for useful coverage. A two-seed
arithmetic smoke run establishes connectivity and state-machine behavior only.
Binary judge calibration, local snapshot contamination checks and a real
50-proposal worked funnel are implemented. Curated benchmark ownership,
automatic monitoring/cleanup, learned priors and repeated ignition trials remain
separate work. See the acceptance map for exact tests and the recorded example
for incumbent/candidate H intervals and the actual rejection.
