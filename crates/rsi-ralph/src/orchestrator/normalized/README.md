# Normalized proposal funnel

`run_proposal_h` accepts only `normalized_h` predictions and uses immutable host
family references and bootstrap settings from `HEpoch.policy`. The effective
margin is the greater of the host margin and the proposal's normalized minimum
gain. The legacy `run_proposal` still uses raw verified quality and is not an H
adapter. `HEvaluator` extends its memory-pinning port but supplies H evidence
through `evaluate_h`; its raw `evaluate` method is never called by this funnel.

`HEpoch.suite_ids` freezes distinct Screen, Selection, and Confirmation suite
fingerprints before execution. The returned `HEvalPair` must identify the exact
expected suite, pair identical task/family/seed keys, and carry measured health
rates and cost ratios. Host code owns the private suites, denominators, prompt
and output caps, memory isolation, and calibrated verifiers. No private oracle
or task content appears in the proposer-facing types.

The language apply/guard, verifier-acceptance protection, read-only memory rules,
watermark advancement, grounded references, class staging, and exact deduplication
are shared with the legacy funnel. A valid snapshot is archived before any
model execution. Invalid data, unknown suite IDs, execution failures, and
unavailable approval stop without promoting it. Completed evidence remains in
the archive even when a later stage fails.

Every completed comparison is persisted as `NormalizedEvaluationEvent`, with
exact proposal, parent and candidate IDs, frozen suite map, cap, serializable
policy, paired runs, health rates, and `ScoreComparison`. Replay rebuilds the
policy through validation and recomputes the paired bootstrap. Confirmation
must reuse neither tasks nor seeds from screening and selection combined.
The human gate sees normalized evidence directly. An explicit decision is
persisted before the final language record; the normalized objective in that
record must exactly equal the candidate's measured H. Replay rejects normalized
accepted records without a valid positive confirmation and human approval.

Archives containing normalized events use schema version two. Legacy version
one archives remain readable. Prepared snapshots and partial measurements are
not automatic resumable checkpoints: host recovery must also bind epoch,
incumbent, suite contents, scoring configuration, memory state, pending calls,
and possible spent budget. The funnel returns an activation boundary, never
mutates the live Coordinator, and makes no ignition claim.

The runner now writes a normalized preregistration before any memory pinning or
evaluator callback. Replay derives the prediction-adjusted margin from that
frozen base policy and rejects same-epoch configuration contradictions,
post-output policy/suite replacement, or a declaration moved behind preparation.
Matching already-observed patch candidates stop as Duplicate before pinning.
Neither a prepared snapshot nor a direct Record call can bypass normalized
confirmation and approval for a preregistered candidate.
