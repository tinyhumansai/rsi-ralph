# RSI loop visualizer implementation

Spec: [RSI loop visualizer](../specs/rsi-viz.md).

Assumes the `events.jsonl` schema from the script-loop PR is fixed. The app is
a standalone Next.js (App Router, TypeScript) package in `apps/rsi-viz/` with
no chart or CSS framework; charts are hand-written SVG.

## Tasks

1. **Scaffold.** `apps/rsi-viz/.gitignore` first (`node_modules/`, `.next/`),
   then `package.json` with `dev`, `build`, `start`, `lint` (ESLint flat
   config with `eslint-config-next`), `typecheck`, and `test` (vitest).
   Commit `package-lock.json`.
2. **Wire types.** `lib/events.ts` mirrors the trace schema and a
   `parseEnvelope` validator. Test: rejects unknown kinds and malformed lines.
3. **Line splitter.** `lib/lines.ts`: `LineSplitter.push(chunk)` returns
   complete lines and buffers the tail; `reset()` on truncation. Tests: a line
   split across chunks, several lines in one chunk, CRLF, trailing partial.
4. **Tailer.** `lib/tail.ts`: reads from a byte offset, detects shrink or inode
   change and restarts at 0, driven by `fs.watch` plus a 500 ms poll. Tests
   against a real file in a vitest temp directory: append, partial append,
   truncate, recreate.
5. **Reducer.** `lib/reducer.ts`: events to run state (stage, epochs,
   incumbent chain, candidates with decisions, metric series, finish reason).
   Proposals pair with candidates by `ideated.candidate`. Tests: the schema
   example, an accept and a reject, a `null` candidate, out-of-order
   execution, `null` metrics, reset.
6. **Path guard.** `lib/paths.ts`: validates run ids and snapshot names,
   resolves with `realpath`, and requires containment in the root. Tests:
   `..`, absolute, NUL, symlink escape, allowlist.
7. **Discovery.** `lib/runs.ts`: bounded-depth walk. Test on a temp tree.
8. **Routes.** `/api/runs`, `/api/runs/[id]/events` (SSE with heartbeat and
   abort cleanup), `/api/runs/[id]/snapshot/[file]`.
9. **UI.** Run list; trace view (ring, header, chart, feed, lineage); snapshot
   view (`experiment-report.json` table and CI chart, raw JSON fallback).
10. **Fixtures and simulator.** `fixtures/curve-fit/events.jsonl`,
    `fixtures/hive-2026-10-11/experiment-report.json`, and
    `scripts/simulate-run.mjs`.
11. **CI.** `.github/workflows/rsi-viz.yml`, path-scoped to `apps/rsi-viz/**`.
12. **Docs.** `apps/rsi-viz/README.md`, index entries, root README link.

## Verification

```sh
cd apps/rsi-viz
npm ci && npm run lint && npm run typecheck && npm test && npm run build
RSI_RUN_ROOT=./fixtures npm run dev &
node scripts/simulate-run.mjs   # watch /runs/live-... update
```

From the repository root, `cargo fmt --all -- --check` and
`cargo build --all-targets --all-features` stay green (no Rust changes).

## Checklist

- [x] Spec and plan
- [x] Scaffold, types, splitter, tailer, reducer, path guard, discovery
- [x] Routes and UI
- [x] Fixtures, simulator, CI, docs
