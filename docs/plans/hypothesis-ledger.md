# Hypothesis ledger and diverse slots: implementation plan

Specification: [`../specs/hypothesis-ledger.md`](../specs/hypothesis-ledger.md).
Builds on the [script loop](script-loop.md) and the [loop trace](loop-trace.md).

## Assumptions

- The loop keeps ideating and executing one slot at a time; earlier slots of
  an epoch are visible to later ones through the ledger.
- No new error variants: refusals reuse `InvalidRequest` (model request),
  `InvalidScriptSubject` (policy) and the existing proposal errors.

## Tasks

Each task starts with a failing test beside the code it changes.

1. **Temperature on the model path** (`crates/tinyrsi-model/src/model/`).
   Test that `ModelRequest` defaults to `0.0`, that `with_temperature` accepts
   `0` and `2` and refuses `-0.1`, `2.1`, `NaN` and infinity with
   `InvalidRequest`, and that `OpenRouterModel::body` sends the request's
   temperature. Add the field, the builder and the accessor; update the
   module README.
2. **Proposal fields** (`crates/tinyrsi-evolve/src/script/`). Pin the serde
   shape of `ScriptProposal` with `approach` and `differs_from_previous`, and
   decode a reply without either. Test `approach_label()` (blank, collapsed
   whitespace, 60-character cut). Require `approach` in the tool schema with
   `maxLength: 60`. Add `ScriptBrief::temperature` and test that the
   supervisor's request carries it, and carries `0.0` when unset. Mention the
   label and the resubmission rule in the system prompt.
3. **Trace field** (`crates/tinyrsi-trace/src/trace/`). Pin `ideated` with and
   without `approach`; the spec's example lines stay byte-identical. Add
   `approach: None` to the hive trace in `tinyrsi-host`.
4. **Policy and ledger types** (`crates/tinyrsi/src/script_loop/types.rs`).
   Add `SlotDirective`, `LoopPolicy::{slots, approach_context}` with the
   default directives, and the three optional `LedgerCandidate` fields with
   `skip_serializing_if`. Pin a legacy ledger row and a full one. Test the
   policy refusals (blank instruction, bad temperature).
5. **Approaches summary** (`crates/tinyrsi/src/script_loop/approaches.rs`,
   tests in `test_approaches.rs`). Normalization, grouping, best mean per
   direction, verdict precedence, recency order and the bound; render the
   section and the earlier-slot section.
6. **Prompt and loop** (`prompt.rs`, `mod.rs`). Render the approaches section
   above the rejections, the earlier-slot section and the slot directive last.
   The loop passes the slot's directive and temperature, stores the new ledger
   fields and emits `approach` on `ideated`. End-to-end test: slot 1's prompt
   names slot 0's approach and decision; the requested temperature reaches the
   model; the ledger and trace carry the approach.
7. **Examples.** `tinyrsi_examples::with_approach` adds a label to a canned
   reply; every canned reply in `math_problem`, `upgrade_file`,
   `graph_coloring` and `crates/tinyrsi/examples/script_loop` names its
   approach. Each still reaches its goal.
8. **Visualizer** (`apps/rsi-viz`). Validate the optional `ideated.approach`
   (string, `null` or absent), carry it in the reducer, show it in the
   proposals feed, add it to a fixture line, and test all three.
9. **Docs.** This plan and the spec indexed; `docs/specs/script-loop.md`,
   `docs/specs/loop-trace.md`, `docs/guides/script-loop.md`, the script loop
   and model READMEs, the evolve crate docs; roadmap items 1 and 2 marked
   implemented.

## Verification

```sh
cargo +1.99 fmt -- --check
cargo +1.99 clippy --all-targets --all-features -- -D warnings
cargo +1.99 build --all-targets --all-features
cargo +1.99 test --all-features
RUSTDOCFLAGS="-D warnings" cargo +1.99 doc --no-deps --all-features
cargo +1.99 run -p tinyrsi --example script_loop
cargo +1.99 run -p tinyrsi-examples --example math_problem   # and upgrade_file, graph_coloring, hive_goal
(cd apps/rsi-viz && npm ci && npm run lint && npm run typecheck && npm test && npm run build)
```

## Checklist

- [ ] Temperature on the model path
- [ ] Proposal fields, brief temperature, schema
- [ ] Trace field
- [ ] Policy and ledger types
- [ ] Approaches summary
- [ ] Prompt and loop
- [ ] Examples
- [ ] Visualizer
- [ ] Docs
