# Crate split implementation plan

Specification: [`../specs/crate-split.md`](../specs/crate-split.md).

Each task is one pull request, merged in order. Each starts from a freshly
fast-forwarded `main` and leaves the four contract commands green. Moves are
`git mv` of whole module directories, so history follows the files. Path fixes
are mechanical (`crate::X` → `tinyrsi_<crate>::X`). Behavior changes land in
separate commits from moves.

## Tasks

1. **Drop the bus and the module.**
   - Delete `crates/tinyrsi-bus`, `src/tinybus_module/`, the `verify_module`
     and `verify_github_release` examples, `MODULE.md` and the
     `vendor/tinybus` submodule.
   - Delete the `tinybus` and `tinybus-module` workspace dependencies.
   - Set `crate-type` back to the default.
   - Move `GradeEntry` and `Verdict` into `lineage/types.rs`, keeping their
     serde pins. Remove the crate-root `append`/`head` bus wrappers.
   - Re-export the hive types from `tinyhivemind_lang`.
   - Remove the transport-free CI step.
   - Reduce `release.yml` to validate, bump, tag and release.
   - Update `AGENTS.md`, `README.md`, `ROADMAP.md`, `docs/architecture.md`,
     `docs/README.md` and the root manifest comments.
   - Mark the `tinybus-module-release` spec and plan superseded.
2. **`tinyrsi-types` and `tinyrsi-model`.**
   - Move `Proposal`, `Prediction`, `CandidateState`, `ReasonCode`,
     `EvalStage` and `ProposalEvent` with their serde tests.
   - Move `model/` and the script supervisor's `block_on`.
   - Archive replay tests must pass unchanged.
3. **`tinyrsi-trace`.** Move `trace/`, then add `TracingTrace` (test: a
   subscriber captures one event per `LoopEvent`) and `FanoutTrace`. The
   `example.jsonl` byte pins and `apps/rsi-viz` `npm test` stay green.
   Then add **`tinyrsi-langfuse`**: the `vendor/openhuman` submodule, the
   OpenHuman `[patch]` tables at the root, and `LangfuseTrace` over
   `openhuman_embed::observe::langfuse::LangfuseClient`, tested against a
   loopback HTTP listener.
4. **`tinyrsi-score`.**
   - Move `evaluation/`, `scoring/` and `experiment/`.
   - Extract `script_loop/score.rs` as `MetricScorer` plus `decide`.
   - Test first: `MetricScorer` monotonicity, `decide` parity with the old
     `score` (decision and metric-unit interval), `LlmJudge` median and
     malformed reply, the uncalibrated judge never accepting, the eligibility
     gate inside a `Composite`, `BackroomsJudge` with `ScriptedConversant`, and
     `Composite` weights.
5. **`tinyrsi-verify`.** Move `process/`, `blueprint/` and `verification/`, then
   add `VerifierScorer` with pass and fail tests.
6. **`tinyrsi-evolve`.**
   - Move `retreat/`, `retreat_memory/` and `script_loop/supervisor.rs` as
     `ScriptSupervisor`.
   - Add the `Evolver` port with its associated `Proposal` type,
     `RetreatEvolver` and `ScriptedEvolver`. Materializing a proposal stays
     in the loop.
   - `retreat` takes a `RetreatDossier` view instead of `observation::Dossier`.
7. **Core and host.**
   - Rewire `script_loop` onto `Scorer` and `Evolver`.
   - Move the `host`-gated modules (`host`, `suites`, `epoch_driver`,
     `evidence_journal`) into `tinyrsi-host` and drop the feature. `memory` and
     `observation` stay in core; the `MemorySnapshot` implementation moves to
     the host crate.
   - Point the host examples and the CI coverage script at the new crate.
   - Emit `LoopEvent`s from `epoch_driver`.
8. **Examples.**
   - Add the `examples/` member with `math_problem`, `upgrade_file` and
     `hive_goal`.
   - Each runs offline with scripted ports, and live with
     `TINYRSI_LIVE_OPENROUTER=1`.
   - Add `examples/README.md`, and add a CI step that runs all three.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
cargo test -p <each crate>
cargo tree -p tinyrsi-score   # no tinyrsi, no tinyhivemind-hives
.github/scripts/check-file-coverage.sh
```

## Checklist

- [ ] 1. Bus and module removed
- [ ] 2. Types and model
- [ ] 3. Trace
- [ ] 4. Score
- [ ] 5. Verify
- [ ] 6. Evolve
- [ ] 7. Core and host
- [ ] 8. Examples
