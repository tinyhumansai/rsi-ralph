# Loop trace

**Status:** Accepted. **Owner:** RSI Ralph maintainers.

## Problem

Every RSI Ralph artifact before this one is a snapshot: an atomically rewritten
JSON document behind an exclusive lock. Nothing tells a watcher *what the loop
is doing right now*, so a run can only be inspected after it ends, and a
visualizer would have to take the same locks the writer holds.

## Goals and non-goals

- Goal: one append-only event stream per run that a separate process can tail
  without locks, shared by the script loop and the hive `complete` example.
- Goal: deterministic timestamps in tests through an injected clock.
- Non-goal: replacing the durable snapshots. The trace is observability, not
  evidence; recovery never reads it.
- Non-goal: a network transport. Readers tail the file.

## Proposed behavior

The `trace` module exposes `LoopEvent`, the `TraceRecord` envelope, a `Clock`
trait (`MonotonicClock`, `ManualClock`), and a `TraceSink` trait with three
sinks: `JsonlTrace` (the file below), `VecTrace` (in memory) and `NullTrace`
(discards). A sink failure surfaces as `Error::TraceIo`.

## Wire schema

The schema below is fixed and shared with the visualizer. Field names, kinds
and their order are part of the contract; `trace/test.rs` pins every example
line byte for byte.

File: `<run-dir>/events.jsonl`. Append-only, UTF-8, one JSON object per line,
`\n`-terminated, flushed after every line. A reader may see a partial final
line while a write is in flight and must buffer it until the newline arrives.

Envelope (every line):

```json
{"seq": 0, "elapsed_ms": 0, "event": { "kind": "<snake_case>", ...fields }}
```

- `seq`: u64, starts at 0, +1 per line, no gaps.
- `elapsed_ms`: u64 milliseconds since the run started (injected clock).
- Candidate ids are short content ids (hex strings, e.g. 12 chars).
- Numbers that may be absent are `null`, never omitted.

Event kinds (stage in brackets is the visualizer's loop-ring stage):

| kind | fields | stage |
|---|---|---|
| `run_started` | `run_id: string, subject: string, goal: {metric: string, direction: "minimize"\|"maximize", target: number\|null}` | — |
| `epoch_started` | `epoch: u64, incumbent: string` | — |
| `observed` | `epoch, incumbent: string, incumbent_metric: number\|null, history: u32, rejected: u32` | observe |
| `ideated` | `epoch, proposal_id: string, parent: string, candidate: string\|null, rationale: string, hypothesis: string, approach?: string` | ideate |
| `executed` | `epoch, candidate: string, seed: u64, metric: number\|null, ok: bool, duration_ms: u64` | execute |
| `measured` | `epoch, candidate: string, mean: number\|null, failures: u32, seeds: u32` | measure |
| `scored` | `epoch, candidate: string, decision: "accept"\|"reject"\|"inconclusive", estimate: number, lower: number, upper: number, units?: "quality"` | score |
| `improved` | `epoch, from: string, to: string, metric: number` | improve |
| `epoch_finished` | `epoch, incumbent: string, incumbent_metric: number\|null` | — |
| `provider_refused` | `epoch, calls: u32, refused: u32, status: u16, message: string` | — |
| `run_finished` | `reason: "goal_reached"\|"max_epochs"\|"stalled"\|"budget"\|"error"\|"provider_refused", incumbent: string, incumbent_metric: number\|null` | — |

`ideated.candidate` is the content id the proposal materializes into, computed
from the proposed files before execution, or `null` when the proposal was
refused before materialization (malformed, or a path outside the editable set).
`ideated.approach` is the proposal's short technique label (see the
[hypothesis ledger](hypothesis-ledger.md)). It is an additive field, omitted
when the proposal has no label or was refused, so unlabelled traces (and the
hive loop's) keep their exact bytes; readers treat a missing field and `null`
alike.

`provider_refused` is written by the `rsi-ralph` binary's engine, just before
`epoch_finished`, when the model provider refused calls during an epoch whose
hive changed nothing (or refused every call). `calls` counts the hive's calls
forwarded to the provider that epoch, `refused` those answered with a
non-success status or never answered, and `status`/`message` describe the
latest refusal (`502` when the provider was unreachable), scrubbed of key
values. When every call was refused the run ends with reason
`provider_refused`; an ordinary empty epoch, with no refusals, writes no such
event and still counts towards `stalled`.

The baseline run of the starting script is epoch 0: `executed`/`measured`
events with `candidate` = the baseline id, and no `scored` event.
`scored.estimate/lower/upper` are the paired candidate-minus-incumbent
improvement in the "higher is better" orientation (for minimize metrics the
sign is flipped, so positive always means better). They are in the goal metric's units
unless `scored.units` is `"quality"`, which a judge or blended scorer reports
on its `[0, 1]` quality scale. `units` is omitted for metric units, so traces
from the metric scorer keep their exact bytes, and a missing field reads as
`"metric"`.

Example run:

```jsonl
{"seq":0,"elapsed_ms":0,"event":{"kind":"run_started","run_id":"curve-fit-1","subject":"curve_fit.py","goal":{"metric":"rmse","direction":"minimize","target":0.05}}}
{"seq":1,"elapsed_ms":3,"event":{"kind":"executed","epoch":0,"candidate":"a1b2c3d4e5f6","seed":1,"metric":0.42,"ok":true,"duration_ms":120}}
{"seq":2,"elapsed_ms":4,"event":{"kind":"measured","epoch":0,"candidate":"a1b2c3d4e5f6","mean":0.42,"failures":0,"seeds":1}}
{"seq":3,"elapsed_ms":5,"event":{"kind":"epoch_started","epoch":1,"incumbent":"a1b2c3d4e5f6"}}
{"seq":4,"elapsed_ms":5,"event":{"kind":"observed","epoch":1,"incumbent":"a1b2c3d4e5f6","incumbent_metric":0.42,"history":1,"rejected":0}}
{"seq":5,"elapsed_ms":900,"event":{"kind":"ideated","epoch":1,"proposal_id":"p1-0","parent":"a1b2c3d4e5f6","candidate":"0f9e8d7c6b5a","rationale":"linear model underfits","hypothesis":"cubic features cut rmse"}}
{"seq":6,"elapsed_ms":1100,"event":{"kind":"executed","epoch":1,"candidate":"0f9e8d7c6b5a","seed":1,"metric":0.11,"ok":true,"duration_ms":130}}
{"seq":7,"elapsed_ms":1101,"event":{"kind":"measured","epoch":1,"candidate":"0f9e8d7c6b5a","mean":0.11,"failures":0,"seeds":1}}
{"seq":8,"elapsed_ms":1102,"event":{"kind":"scored","epoch":1,"candidate":"0f9e8d7c6b5a","decision":"accept","estimate":0.31,"lower":0.28,"upper":0.34}}
{"seq":9,"elapsed_ms":1102,"event":{"kind":"improved","epoch":1,"from":"a1b2c3d4e5f6","to":"0f9e8d7c6b5a","metric":0.11}}
{"seq":10,"elapsed_ms":1103,"event":{"kind":"epoch_finished","epoch":1,"incumbent":"0f9e8d7c6b5a","incumbent_metric":0.11}}
{"seq":11,"elapsed_ms":1104,"event":{"kind":"run_finished","reason":"max_epochs","incumbent":"0f9e8d7c6b5a","incumbent_metric":0.11}}
```

## Invariants and constraints

- `JsonlTrace` creates its parent directory, refuses to reuse an existing file
  (a fresh run starts at `seq` 0), and flushes after every line.
- Non-finite numbers are never written: an absent metric is `null`.
- Writing an event never blocks on a reader and never takes a lock.

## Acceptance criteria

- Each example line above deserializes into a `TraceRecord` and serializes back
  to the identical bytes.
- `JsonlTrace` numbers lines from 0 without gaps and every line is
  newline-terminated and readable while the writer is still open.

## Open questions

None.
