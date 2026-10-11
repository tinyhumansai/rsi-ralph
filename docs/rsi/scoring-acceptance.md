# Scoring acceptance and implementation map

Issue [#2](https://github.com/tinyhumansai/tinyrsi/issues/2) is a planning/spec
issue. Its normative deliverable is the [scoring specification](specs/scoring.md),
with a [worked recorded-data example](scoring-worked-example.md). Acceptance of
that design does not assert that every intended mechanism below already exists.
Future implementation must add the listed behavioral tests before claiming
completion. No tests are skipped or added to pretend a missing feature exists.

## Current regression suites

The identifiers in the tables refer to these actual source files:

- **S**: [normalized scoring tests](../../crates/tinyrsi/src/scoring/test.rs).
- **N**: [normalized proposal tests](../../crates/tinyrsi/src/orchestrator/normalized/test.rs).
- **O**: [original proposal tests](../../crates/tinyrsi/src/orchestrator/test.rs).
- **F**: [memory/guard funnel tests](../../crates/tinyrsi/src/orchestrator/test_funnel.rs).
- **Q**: [quality and judge-calibration tests](../../crates/tinyrsi/src/evaluation/test.rs).
- **M**: [snapshot-memory tests](../../crates/tinyrsi/src/memory/test.rs).
- **E**: [Engine epoch tests](../../crates/tinyrsi/src/epoch_driver/test.rs).

## Reject-code coverage and compatibility

The current public [ReasonCode](../../crates/tinyrsi/src/orchestrator/types.rs)
serializes twelve outcomes. The intended seventeen rejection codes below are a
future more detailed taxonomy, not a change to that wire contract in this PR.
Current evaluation evidence retains detailed decision explanations even when
its top-level reason is coarse. Current reason strings in archive events may
use Rust variant spelling, while serialized `ReasonCode` uses screaming snake
case; consumers must not confuse those representations.

| Intended code | Current behavior / existing test | Remaining implementation test |
| --- | --- | --- |
| `SCHEMA` | Invalid Engine occurrence counter: E `actual_engine_counts_invalid_attempt_and_retains_durable_discovery` | Persist each invalid/repair occurrence's safe ID, stage and code |
| `UNFORMALIZABLE` | O `rejects_invalid_predictions_wrong_classes_and_stale_operations_without_models` | Preserve detailed formalization subtype when new taxonomy ships |
| `INVALID_REF` | Invalid patch targets fold into `UNFORMALIZABLE`; same O test | Distinguish dangling references from stale preconditions |
| `INVARIANT` | Invalid candidate folds into `UNFORMALIZABLE`; formalization rejects | Exercise each invariant with an explicit subtype, no evaluation calls |
| `GUARD` | N `duplicate_and_guarded_normalized_attempts_keep_their_history` | Keep protected-attempt counters with explicit denominators |
| `MEMORY_REACH` | F `pinning_failures_and_unrelated_memory_changes_are_guarded` | Emit distinct reach code for each eval/orchestrator namespace |
| `CONTAMINATION` | M `refuses_stale_entries_readonly_targets_canaries_and_conflicting_snapshots` | Distinct contamination code; durable halt/invalidation tests below |
| `DUP_ACCEPTED` | Exact hash/content identity folds into `DUPLICATE`; the cited N duplicate test exercises rejected history, not an accepted-history case | Explicit accepted-history regression; distinguish accepted history and frozen embedding near-duplicates |
| `DUP_REJECTED` | O `screen_stops_losers_and_duplicate_proposals_stay_in_history` | Distinguish rejected history and require recorded new evidence |
| `UNTESTABLE` | N `normalized_configuration_and_prediction_validation_precede_execution` | Extend prediction schema only with versioned compatibility tests |
| `UNGROUNDED` | O `rejects_ungrounded_and_guarded_proposals_before_execution` checks existence | Existing-but-irrelevant references must fail semantic support checks |
| `NO_EFFECT` | Generic paired Screen exists; rejection folds into `NO_GAIN` | Assert targeted failures improve under frozen replay criteria |
| `CANARY_REGRESSION` | Family gates exist; there is no separate targeted canary-set verdict | Isolate canary regression from aggregate replay loss |
| `NO_GAIN` | N `h_uncertainty_and_all_health_cost_gates_prevent_promotion` | Keep no-gain distinct from constraints when taxonomy expands |
| `CONSTRAINT` | Folds into `NO_GAIN`, retaining comparison reason: N `aggregate_h_gain_cannot_hide_a_family_regression` | Emit distinct code for every family/health/cost gate |
| `NOT_REPLICATED` | N `raw_predictions_reused_confirmation_and_health_regressions_are_rejected` | Continue testing fresh tasks AND seeds, not just one dimension |
| `WRONG_CLASS` | O `rejects_invalid_predictions_wrong_classes_and_stale_operations_without_models` | Preserve authorized-class checks as classes expand |

`DUPLICATE` also covers already-prepared content identities before spending
evaluation budget: F `already_prepared_candidates_are_detected_before_spending_evaluation_budget`.
Host failure, approval denial, inconclusive and accepted dispositions are tested
separately; none should become a measured eligibility label just because it is
present in the archive.

## Decision rows and required tests

| Decision row | Implemented evidence | Remaining controller test |
| --- | --- | --- |
| Eligible gain → confirmation → human approval → acceptance | N `normalized_confirmation_is_required_before_human_gate_and_promotion` | Broader host adapters retain exact-candidate approval |
| Negative upper ΔH → reject | N `h_uncertainty_and_all_health_cost_gates_prevent_promotion`; recorded screen loss | Separate `NO_GAIN` after taxonomy expansion |
| Margin overlap/equality → inconclusive | S `zero_token_reference_and_tie_decisions_are_explicit` | Valid alpha-spending looks, maximum spend and stopping/restart identity |
| H gain with family/health/cost violation → reject | S `rejects_normalized_family_health_and_cost_regressions`; N `aggregate_h_gain_cannot_hide_a_family_regression` | Distinct `CONSTRAINT` label for every gate |
| Confirmation failure/reuse → no approval | N `raw_predictions_reused_confirmation_and_health_regressions_are_rejected` | No automatic confirmation retry on the same draw |
| Sustained anchor decline / wider gap → rollback/freeze | S `anchor_ledger_detects_sustained_loss_and_widening_gap` implements signal only | Persistent comparable anchor history; eligible ancestor rollback and class freeze |
| Tamper spike → halt/review | Guards exist; no persistent threshold controller | Threshold crossings, no new calls/deployments, restart retains halt |
| Canary in memory → halt/forget/invalidate | M contamination rejection and reversible learning exist | Scan actual contents; quarantine all affected snapshots, event cleanup, anchor invalidation, clean remeasurement |
| Near-zero class acceptance → change search | No automated class-window controller | Frozen window/threshold, efficacy versus approval-denial denominators, authorized class/mode changes |
| Prior degradation → more uniform testing | No calibrated prior controller | Frozen Brier/rank/precision thresholds and deterministic allocation changes |
| Approval denial/unavailable or host failure → retain incumbent | N `denied_or_failed_approval_does_not_advance_head`; N `mismatched_suite_invalid_evidence_and_host_errors_fail_closed` | New adapters preserve failure labels and prevent implicit retry |

The host already has explicit activation, parent rollback and recovery APIs;
that does not automatically connect them to anchor/tamper/class-window advice.
An inconclusive screen may continue to selection, but inconclusive selection
stops. No current percentile-bootstrap test asserts anytime-valid inference.

## Scoring, vague domains and dataset requirements

| Mechanism | Evidence / required future test |
| --- | --- |
| Eval-owned quality and verifier tiers | Q `quality_uses_eval_owned_strengths_and_budget_capped_goal`; Q `judge_weights_are_earned_and_preferences_are_discounted` |
| Binary human calibration, invalid/degenerate labels | Q `invalid_quality_rubric_or_calibration_is_rejected` |
| Family normalization, penalties and fractional IQM | S `formula_normalizes_family_quality_and_subtracts_health_penalties`; S `fractional_iqm_handles_one_two_and_three_families` |
| Shared task/seed cluster bootstrap | S `bootstrap_is_reproducible_and_pairs_remove_task_seed_noise`; S `bootstrap_resamples_task_clusters_and_applies_family_weights` |
| Policy, telemetry and pairing validation | S `invalid_anchors_rates_caps_pairing_and_resample_count_are_errors`; S `malformed_policy_telemetry_and_numeric_overflow_are_rejected` |
| Exact zero-tolerance integer cost | S `zero_cost_tolerance_preserves_integer_precision` |
| Monitoring epoch ordering and gaps | S `anchor_monitoring_rejects_bad_evidence_and_resets_skipped_epochs` |
| Frozen policy replay | S `scoring_policy_spec_revalidates_serialized_configuration`; N `replay_rejects_altered_bootstrap_and_normalized_policy` |
| Pinned memory and restoration | M `learning_is_pinned_without_changing_live_memory_and_restores_after_reopen` |
| Geometric diagnostic | Future: positive, zero and negative/undefined families; no clipping |
| Pairwise prior / subjective panel | Future: swapped positions, blindness, ties, abstentions, disconnected Bradley–Terry graph, frozen judge versions |
| Calibrated EV / successive halving | Future: invalid/zero costs, uncensored eligibility labels, frozen round limits, exploration share, smaller-patch ties |
| Outcome dataset and metrics | Future: per-occurrence source/prior rows, missing/censored labels, Brier/Spearman/precision@k, no confirmation/anchor leakage |
| Ordinal calibration / human collection | Future: rubric version, label provenance, sample size, disagreement, zero-trust judge and private-data boundaries |
| Memory health / attribution | Future: writer identity after reassignment, recall-use/clipping/latency denominators, growth, contamination lineage |

## Acceptance evidence

The specification now defines the complete funnel, H, decision table,
vague-domain calibration, memory attribution and outcome-learning dataset.
The recorded example supplies fifty typed retreat occurrences, a real paired
incumbent/candidate H interval and its no-gain decision. A separate controlled
proposal supplies confirmation/approval/deployment evidence without replacing
any failed attempt. Current implemented decisions have regression suites;
future mechanisms have explicit test obligations above.

Implementation follow-ups belong to the existing
[P2 human-gated loop](https://github.com/tinyhumansai/tinyrsi/issues/4),
[P4 retreat](https://github.com/tinyhumansai/tinyrsi/issues/6), and
[P6 experiment](https://github.com/tinyhumansai/tinyrsi/issues/8) trackers, with
memory-engine cleanup remaining upstream/host work. This planning issue's
closure does not close those implementation or scientific acceptance trackers.
