# RSI loop visualizer

**Status:** Accepted. **Owner:** TinyRSI maintainers.

Plan: [`../plans/rsi-viz.md`](../plans/rsi-viz.md).

## Problem

A running self-improvement loop writes its evidence to disk: the script loop
appends a structured trace to `events.jsonl`, and the hive epoch driver leaves
JSON snapshots (`archive.json`, `epoch.json`, `experiment-report.json`, ...).
Reading those files by hand while a loop is running does not show where the
loop is, whether the metric is moving toward the goal, or why a candidate was
rejected.

## Goals

- A local, read-only web app, `apps/rsi-viz/`, that lists runs under one root
  directory and renders each one.
- Live view of a trace run: the stage the loop is in, the goal, the incumbent,
  the metric per epoch, every proposal with its decision and confidence
  interval, and the chain of incumbents.
- Static view of snapshot-only runs, with a dedicated rendering of
  `experiment-report.json` and a raw JSON fallback for other snapshot files.
- Demoable without a Rust build, from checked-in fixtures.

## Non-goals

- Writing to, locking, or controlling a run. The app never starts, stops, or
  steers a loop.
- Authentication or multi-user deployment. It is a localhost tool.
- Rendering private task data. It shows only what the run directory holds.

## Inputs

`RSI_RUN_ROOT` names the root directory; it defaults to `../../target`
resolved from the app directory (the workspace `target/`). Discovery walks the
root to depth 3 and reports:

- a **trace run** for every directory holding `events.jsonl`;
- a **snapshot run** for every other directory holding `archive.json`,
  `epoch.json`, or `experiment-report.json`.

A run id is the directory path relative to the root, with `/` separators.

The trace wire format is owned by the script-loop PR (`docs/specs/loop-trace.md`
once it lands): one envelope `{seq, elapsed_ms, event}` per line, where
`event.kind` is one of `run_started`, `epoch_started`, `observed`, `ideated`,
`executed`, `measured`, `scored`, `improved`, `epoch_finished`,
`run_finished`. Optional values are `null`, except the additive fields
`scored.units` and `ideated.approach`, which are omitted when absent (a
reader treats missing and `null` alike). The visualizer mirrors this schema
exactly in `lib/events.ts` and adds no fields.

Proposals are paired with candidates by `ideated.candidate`, the content id
the proposal materializes into; it is `null` when the proposal was rejected
before materialization, and such a proposal is shown as not executed. A
proposal with an `ideated.approach` label shows it next to its id.

## HTTP surface

All handlers are dynamic, Node.js runtime, and read-only.

| Route | Response |
|---|---|
| `GET /api/runs` | JSON array of `{id, kind: "trace" \| "snapshot", updatedAt, files}` |
| `GET /api/runs/{id}/events` | `text/event-stream`: one `data:` message per complete line, replay first, then live tail; `: ping` heartbeats |
| `GET /api/runs/{id}/snapshot/{file}` | the JSON file, only when allowlisted |

The snapshot allowlist is `archive.json`, `epoch.json`,
`experiment-report.json`, `ledger.json`, `deployment.json`, and
`<name>.generation-<N>.json`.

## Invariants

- **Path safety.** A run id or file name containing `..` segments, a NUL
  byte, or an absolute path is rejected with 400. The resolved real path of
  every opened file must stay inside the real path of `RSI_RUN_ROOT`, so a
  symlink cannot escape it.
- **No writes.** The app opens run files read-only and never creates lock or
  sidecar files.
- **Line integrity.** A partial trailing line is buffered until its newline;
  a reader never emits half a JSON object. A line that does not parse is
  surfaced as an error count, not a crash.
- **Truncation.** If the file shrinks or is replaced, the tailer restarts
  from offset 0 and the client resets its state.
- **Clean shutdown.** Closing the browser tab stops the watcher and timers.

## UI

- Run list: id, kind, last update.
- Trace run: loop ring (observe, ideate, execute, measure, score, improve)
  highlighting the latest event's stage, pulsing while live; header with goal
  (metric, direction, target), status (live / finished with reason) and the
  incumbent; metric-vs-epoch chart (incumbent step line, candidate points
  coloured by decision, dashed target line); proposals feed, including
  proposals that were never executed; incumbent lineage.
- Snapshot run: arm by ablation table of trial counts with anchor gain, a CI
  dot-and-whisker chart of anchor intervals, and raw JSON for other files.
- Light and dark colour schemes; usable at phone width.

## Acceptance criteria

1. `npm run lint`, `typecheck`, `test`, and `build` pass in `apps/rsi-viz`.
2. Unit tests cover the line splitter (partial lines, multi-chunk, truncation),
   the event reducer, and the path guard.
3. With `RSI_RUN_ROOT=./fixtures`, the fixture trace and the 2026-10-11
   experiment report render, and `scripts/simulate-run.mjs` produces a run
   whose page updates live.
4. Requests for `../`, absolute, or non-allowlisted paths return 400 or 404.

## Open questions

None blocking. Hive epoch driver trace emission is a follow-up; until then
hive runs appear as snapshot runs.
