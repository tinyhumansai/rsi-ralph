# Script loop implementation plan

Specification: [`../specs/script-loop.md`](../specs/script-loop.md). Depends on
the [loop trace](loop-trace.md).

## Tasks

1. **Shared process runner.** Move the body of `verification::process` into a
   private `crates/tinyrsi/src/process/` module that takes program, arguments,
   working directory, scratch directory, environment, timeout and cap, and
   reports `Exited`, `TimedOut` or `OutputExceeded` with separate stdout and
   stderr. Verification maps the result back to its old tuple; its existing
   tests must stay green unchanged. Add `process/test.rs` for the new outcome
   distinctions.
2. **Errors.** Add the specification's new `Error` variants with message tests.
3. **Measure.** Test `parse_metric` for a valid line, a missing line
   (`MetricMissing`), `nan`/`inf`/garbage (`MetricInvalid`) and last-line-wins;
   implement it.
4. **Proposal validation.** Test absolute, `..`, unlisted and empty-file-list
   proposals (`ProposalPathOutsideEditable`, `SupervisorMalformed`); implement
   `ScriptProposal::validate`.
5. **Supervisor.** With a scripted `ModelPort`, test the forced tool request,
   decoding, malformed replies and token accounting; implement `Supervisor`
   with a small thread-parking executor so the loop stays synchronous and
   needs no async runtime.
6. **Execute.** Test materialization, content ids, seeds in the environment,
   cleared environment, timeout (`ScriptTimeout`), output cap, non-zero exit;
   implement candidate materialization and per-seed execution.
7. **Score.** Test the quality transform's monotonicity and the reported
   metric-unit interval; implement scoring over `evaluation::compare`.
8. **Loop.** End-to-end tests with `/bin/sh` fixtures: better-then-worse
   (accept, reject, promotion, trace kinds in order, ledger contents),
   `goal_reached`, `max_epochs`, `stalled`, `budget`, baseline failure
   (`error`), path escape and malformed replies recorded as invalid,
   duplicate content rejected.
9. **Example.** `crates/tinyrsi/examples/script_loop/` with `main.rs`,
   `curve_fit.py`, `README.md`; offline scripted supervisor by default,
   `OpenRouterModel` when `TINYRSI_LIVE_OPENROUTER=1`.
10. **Hive trace.** `examples/complete/driver.rs` and `complete_openrouter.rs`
    write `events.jsonl` at stage boundaries.
11. **Docs.** Module READMEs, `docs/guides/script-loop.md`, root README link,
    spec and plan indexes.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
.github/scripts/check-file-coverage.sh 90 coverage.json
cargo run -p tinyrsi --example script_loop
```

## Checklist

- [x] Process runner shared, verification unchanged
- [x] Errors, measure, proposal validation, supervisor
- [x] Execute, score, loop
- [x] Example and hive trace
- [x] Docs
