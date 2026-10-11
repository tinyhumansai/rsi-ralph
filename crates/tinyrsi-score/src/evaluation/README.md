# Paired evaluation

`compare` validates a complete task-by-seed matrix for both configurations:
identical unique task/family/seed keys, one family per task, at least two shared
seeds, finite quality in `[0,1]`, and the same positive per-run token cap.

The estimator first averages paired quality differences over tasks within each
family and seed, then equally weights families. The reported mean and standard
error use independent seed-level means. Repeating tasks therefore cannot create
artificial precision. The interval is the mean plus/minus the policy's standard
error multiplier (at least two); it is a decision heuristic, not a calibrated
small-sample confidence interval.

Acceptance requires the lower bound to exceed the practical margin. Clear
losers have an upper bound no greater than the margin; uncertain evidence is
inconclusive. A family loss beyond its policy limit, any paired increase in
false claims or tampering, or aggregate token growth beyond the fractional cost
tolerance rejects the candidate regardless of quality gain. Zero cost tolerance
uses exact integer token totals. Hard cap violations are invalid evidence.

`Budget`, `EvalRun`, `EvalPolicy`, `EvalDecision`, and `Comparison` serialize via
serde. Reasons explain each decision. Evidence carries task identities only;
private task text and oracle answers must remain host-owned.

Use a small held-out matrix for cheap screening. A screening acceptance is not
promotion: the host must evaluate on full selection evidence and then a fresh
confirmation matrix with both tasks and seeds disjoint. `ensure_independent`
checks this separation. The host controls execution, budgets beyond tokens,
private-suite curation, independent confirmation, and human approval.

## Eval-owned task quality

`QualityRubric` freezes alpha and ordered `VerifierTier` checkpoints, outside the
hive package. `score` computes issue #2's task-quality formula:
`alpha * goal_verified + (1-alpha) * sum(weight * pass) / sum(weight)`.
Goal credit requires verification within the token cap. A checkpoint pass must
have occurred before exhaustion; an overrun run remains invalid for comparison.
Proof, execution, schema, and reference weights are respectively 1, 0.9, 0.7,
and 0.7. An entirely zero-weight rubric is invalid.

`JudgeCalibration::from_labels` computes Cohen's kappa from paired binary human
and judge labels. Agreement below the host's threshold earns zero weight;
negative agreement earns zero weight. Rubric judges receive the earned weight,
and preference judges receive half of it. Degenerate all-constant panels with
undefined kappa are rejected. These labels and rubric configuration are trusted
host inputs; serde is persistence, not a protection against malicious hosts.

## Precise scope

The comparison is a raw equal-family task-quality delta, not the complete
normalized hive score H proposed in issue #2. It does not implement frozen
baseline/reference normalization, family IQM, penalty rates, task-and-seed
bootstrap intervals, anchor progress curves, sequential alpha spending, or
judge-panel ranking. Its paired standard-error interval is an explicit heuristic
and is particularly uncertain with only two seeds. Independent confirmation
reduces selection bias but does not calibrate that interval or prove ignition.
Family and health gates use observed outcomes; they do not assert statistical
noninferiority. Host suites must supply enough seeds and independent tasks to
support the desired confidence and retain immutable calibration references.
