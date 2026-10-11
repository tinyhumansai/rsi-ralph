# Loop trace

An append-only event stream that tells a watcher what an improvement loop is
doing *now*, without taking any of the locks that guard the durable snapshots.
The wire schema is fixed in [`docs/specs/loop-trace.md`](../../../../docs/specs/loop-trace.md)
and shared with the visualizer; `test.rs` pins every example line from it byte
for byte.

## Design

- `LoopEvent` is one stage boundary (`run_started`, `epoch_started`,
  `observed`, `ideated`, `executed`, `measured`, `scored`, `improved`,
  `epoch_finished`, `run_finished`), serialized internally tagged by `kind`.
- `TraceRecord { seq, elapsed_ms, event }` is one line. Sinks stamp `seq` from
  0 without gaps and `elapsed_ms` from an injected `Clock`.
- `Clock`: `MonotonicClock` for real runs, `ManualClock` (shared, advanced by
  hand) for deterministic tests.
- `TraceSink` has three implementations:
  - `JsonlTrace` writes `events.jsonl`. It creates missing parent
    directories, refuses an existing file so a run always starts at `seq` 0,
    writes each line with a single `write_all` and flushes it.
  - `VecTrace` keeps records in memory.
  - `NullTrace` discards everything.
  - `TracingTrace` mirrors events into the `tracing` ecosystem under the
    `tinyrsi::loop` target: `run_started` opens an `rsi.run` span,
    `epoch_started` an `rsi.epoch` span inside it, and every event becomes an
    `INFO` event carrying `seq`, `elapsed_ms`, `kind` and its wire JSON in
    `event`. `epoch_finished` and `run_finished` close their spans. The host's
    subscriber decides where that goes.
  - `FanoutTrace` forwards each event to several sinks in order and stops at
    the first failure, for example `events.jsonl` plus `TracingTrace`.

## Constraints

- The trace is observability, not evidence. Nothing recovers from it, and a
  missing or truncated trace never invalidates a ledger or archive.
- Readers may see a partial last line while a write is in flight and must
  buffer until the newline.
- Absent numbers are `null`, never omitted; the schema has no optional fields.
- A sink failure is `tinyrsi_trace::Error::Io`, which converts into
  `tinyrsi::Error::TraceIo`; the script loop treats it as fatal.
