# Loop trace implementation plan

Specification: [`../specs/loop-trace.md`](../specs/loop-trace.md).

## Tasks

1. **Pin the wire shape.** In `crates/tinyrsi/src/trace/test.rs`, round-trip
   every example line of the specification through `TraceRecord` and assert
   byte equality. Fails until the types exist.
2. **Types.** `crates/tinyrsi/src/trace/types.rs`: `LoopEvent` (internally
   tagged `kind`, snake case), `TraceRecord`, `RunGoal`, `MetricDirection`,
   `ScoreDecision`, `StopReason`. Field order follows the specification.
3. **Clocks.** Test that `ManualClock` reports what it was advanced to and that
   clones share time; implement `Clock`, `MonotonicClock`, `ManualClock`.
4. **Sinks.** Test that `JsonlTrace` creates parent directories, numbers lines
   from 0, terminates and flushes each line, and refuses an existing file
   (`Error::TraceIo`); that `VecTrace` stamps identically; that `NullTrace`
   accepts everything. Implement `TraceSink`, `JsonlTrace`, `VecTrace`,
   `NullTrace`.
5. **Exports and docs.** Re-export from `src/lib.rs`, add `trace/README.md`,
   index the spec and this plan.

## Verification

```sh
cargo test -p tinyrsi trace
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Checklist

- [x] Wire shape pinned
- [x] Types, clocks, sinks
- [x] Exports, README, indexes
