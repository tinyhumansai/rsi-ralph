# Normalized hive score and anchor monitoring

`ScoringPolicy::new` freezes the host's family anchors, importance weights,
regression limits, penalty coefficients, practical margin, cost tolerance,
bootstrap count, and deterministic seed. Fields are private; there is no hive
patch surface or deserialization path that can change this policy. Each family
requires `0 <= baseline < reference <= 1`, positive importance, and a nonnegative
normalized regression limit. Every observed family must have exactly one
configured anchor, and every configured family must be represented.

`score` consumes `EvalRun.quality`, which can come from eval-owned
`QualityRubric` checkpoints. It computes each family's task-and-seed mean Q,
normalizes `(Q-baseline)/(reference-baseline)`, and returns:

`H = IQM(weight * normalized_family_Q) - mu*false_claim_rate - nu*tamper_rate - lambda*max(0,cost_ratio-1)`

Normalization is not clipped: a candidate may beat its reference or fall below
its baseline. IQM is the exact middle 50% integral of the equally weighted
empirical distribution of weighted family scores. Fractional trimming includes
parts of observations at the boundaries, so one, two, or three families remain
well-defined. With one family it returns that family's weighted score; with two
it returns their mean. Cost savings do not earn a direct bonus.

## Bootstrap and paired decisions

Every report includes a two-sided 95% percentile-bootstrap interval, using
linear interpolation at the 2.5th and 97.5th percentiles. At least 200 resamples
are required; production scientific estimates should use more. Sampling uses a
specified SplitMix64 stream and unbiased bounded rejection sampling, so the same
inputs, policy, and seed produce identical results without network or clocks.

A resample draws task identities with replacement **within each fixed family**,
and independently draws shared seed identities with replacement. Repeated task
and seed clusters preserve the complete matrix. `compare_h` uses exactly the
same draws for incumbent and candidate, then bootstraps their H difference
directly. It does not subtract independent confidence bounds. All run keys,
matrices, quality bounds, and hard token caps use the evaluation module's paired
validation. Importance weights and family references remain fixed.

The bootstrap interval is conditional on fixed host-measured health rates and
references. Rates require claim/action denominators that `EvalRun` does not
carry; the module does not fabricate those denominators or claim to estimate
health-rate uncertainty. A few tasks or seeds can still produce false precision,
and percentile bootstrap intervals are not an alpha-spending sequential test.
Independent confirmation remains necessary after choosing a winner.

`compare_h` rejects any empirical normalized-family regression beyond its
configured limit, any worse false-claim or tamper rate, or excess cost. Candidate
cost telemetry must match aggregate tokens relative to the incumbent within
numerical tolerance; incumbent cost ratio is one. Zero incumbent tokens allow
only a zero-cost candidate, with ratio defined as one. Zero cost tolerance also
compares exact integer token totals, preserving single-token increases above
floating-point precision. Positive tolerances use floating-point ratios.

A lower paired-delta bound above the practical margin is eligible for
acceptance. An upper bound below zero rejects a clear loser. Other evidence is
inconclusive. These results do not promote versions or waive confirmation,
human approval, memory pinning, or host-owned private suites.

## Anchor monitoring

`AnchorLedger` observes monotonically increasing epochs, with selection and
anchor reports measured under the same immutable family references and pinned
memory identity. The host owns that identity and should persist it alongside
`ScoreEstimate`; the score report itself does not carry private task contents
or memory snapshots.

After at least two configured consecutive epochs, the ledger signals:

- regression when anchor upper bounds fall below the best previous anchor's
  lower bound minus the regression margin;
- widening selection-anchor gap when selection lower minus anchor upper
  exceeds the first epoch's point-estimate gap plus its margin.

Skipped epochs reset consecutive streaks. Recovery resets the relevant streak.
The highest anchor point estimate determines `best_epoch`; this is monitoring
advice, not an automatic statistically optimal ancestor choice. No rollback,
class freeze, contamination cleanup, or live activation is performed.

The module implements issue #2's H formula, paired uncertainty, hard gates,
and basic anchor monitoring. Bradley-Terry proposal ranking, learned priors,
sequential alpha spending, canary ownership, calibrated health-rate uncertainty,
and the worked experiment with at least twenty real proposals remain host or
future work. Family references and health-rate definitions must be curated
before real progress claims are made.

`ScoringPolicy::spec()` exports frozen settings as `ScoringPolicySpec` for exact
archive replay. The serializable specification is trusted host configuration;
`build()` revalidates it through the immutable policy constructor. Deserializing
a specification alone never makes malformed anchors or bootstrap settings valid.
Normalized proposal evaluation persists the effective prediction-adjusted policy
and compares H directly, preserving the bootstrap meaning in human approval.
