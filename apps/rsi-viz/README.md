# rsi-viz

A local, read-only web app for watching TinyRSI loops. It lists the runs under
one directory and renders each one:

- **Trace runs** (a directory with `events.jsonl`) update live: a loop ring
  showing the stage of the latest event (observe → ideate → execute → measure
  → score → improve), the goal and incumbent, a metric-by-epoch chart with
  every candidate marked accepted, rejected, or inconclusive, the proposals
  feed with each decision's improvement interval, and the incumbent lineage.
- **Snapshot runs** (a directory with `archive.json`, `epoch.json`, or
  `experiment-report.json`) render statically. `experiment-report.json` gets
  an arm × ablation table and an anchor-interval chart; other allowlisted
  files are shown as JSON.

Spec: [`docs/specs/rsi-viz.md`](../../docs/specs/rsi-viz.md). Plan:
[`docs/plans/rsi-viz.md`](../../docs/plans/rsi-viz.md).

## Run it

Node 20.9 or newer (CI uses 24).

```sh
cd apps/rsi-viz
npm ci
npm run dev            # http://localhost:3000, reads ../../target
```

`RSI_RUN_ROOT` picks the directory to scan (default `../../target`, the
workspace build directory, resolved from this folder). Discovery walks three
levels deep and skips hidden and Cargo build directories.

### Against a live loop

The script loop writes its trace to `target/rsi-script-loop/<run>/events.jsonl`
(the `script_loop` example lands in a parallel pull request):

```sh
cargo run -p tinyrsi --example script_loop     # terminal 1
cd apps/rsi-viz && npm run dev                 # terminal 2
```

Open the run from the list; the page keeps following the file until the loop
writes `run_finished`.

### Without Rust

The fixtures hold a three-epoch curve-fit trace (an accept, a reject, an
inconclusive, then the goal reached) and the 2026-10-11 hive experiment report:

```sh
RSI_RUN_ROOT=./fixtures npm run dev
npm run simulate       # in another terminal: replays the trace into fixtures/live-<time>/
```

`scripts/simulate-run.mjs` takes `--delay <ms>`, `--out <dir>`, and
`--source <events.jsonl>`, and writes some lines in two halves to exercise
partial-line handling. Its output directories are git-ignored.

## How it reads files

| Route | What it does |
|---|---|
| `GET /api/runs` | lists runs as `{id, kind, updatedAt, files}` |
| `GET /api/runs/{id}/events` | Server-Sent Events: replays complete lines, then tails by byte offset |
| `GET /api/runs/{id}/snapshot/{file}` | returns an allowlisted snapshot file |

- The server only opens files for reading and never creates lock files, so it
  is safe beside a running loop.
- The tailer is driven by `fs.watch` on the directory plus a 500 ms poll. A
  partial trailing line is held until its newline. If the file shrinks or is
  replaced, the stream sends `event: reset` and the page starts over.
- Run ids and file names are checked lexically (no `..`, absolute paths, NUL,
  or backslashes), then resolved with `realpath` and required to stay inside
  the real `RSI_RUN_ROOT`, so a symlink cannot escape it.
- The trace carries no explicit proposal-to-candidate link. Within an epoch,
  new candidates are paired with `ideated` proposals in order; re-runs of the
  incumbent are not counted as candidates.

## Layout

```text
app/          # pages and route handlers
components/   # loop ring, charts, feed, lineage, report view
lib/          # pure logic with vitest tests beside each file
fixtures/     # demo runs
scripts/      # simulate-run.mjs
```

The wire types in `lib/events.ts` mirror the Rust `trace` module exactly; when
the schema changes, change both and the fixture together.

## Checks

```sh
npm run lint && npm run typecheck && npm test && npm run build
```

CI runs the same commands in `.github/workflows/rsi-viz.yml` when anything
under `apps/rsi-viz/` changes.

## Limitations

- Localhost tool: no authentication. Do not expose it on a shared network.
- A file truncated and then regrown past the old offset between two polls
  (same inode) is not detected as a reset.
- Hive epoch runs show as snapshot runs until the epoch driver emits a trace.
