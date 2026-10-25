# Score history across runs

**Status:** Implemented. **Owner:** RSI Ralph maintainers.

Related: [upgrade log](upgrade-log.md), [RSI CLI](rsi-cli.md),
[loop trace](loop-trace.md).

## Problem

A loop's progress spans many runs, and each run's UI shows only its own
epochs. To tell whether an upgrade helped, an operator has to open each run,
read off the baseline and candidate scores, and line them up by hand. The
same goes for the time each epoch spends scoring while every seat sits idle,
which is 14 to 16 minutes per epoch in the Erdős 779 runs.

## Goals and non-goals

- Goal: every measured score of a loop, across runs and epochs, in one place.
  That covers each run's baseline, every candidate with its seat, mean,
  failures and seeds, whether it was promoted, and the incumbent it was
  measured against.
- Goal: each epoch's seat time and scoring wall time.
- Goal: each upgrade placed between the run before it and the run it applies
  to.
- Goal: a graph in the `serve` UI, plus text, CSV and JSON from the CLI.
- Non-goal: merging run stores. A loop's runs are read from one
  `$TINYRSI_HOME`.

## Source

The history is derived on demand from the run store. Nothing new is stored,
so it is never out of date and covers runs written before it existed. For
each stored run of the loop, oldest first:

| From `events.jsonl` | Becomes |
| --- | --- |
| `run_started.goal.direction` | `maximize` (`minimize` makes the lowest score best) |
| `measured`, epoch 0 | `baseline`, the incumbent the run started from |
| `epoch_started` | a new epoch with its `incumbent` revision |
| `observed.incumbent_metric` | that incumbent's score |
| `measured`, epoch ≥ 1 | a candidate point |
| `epoch_finished.incumbent_metric` | `finished_metric` |

Seats and promotion come from the run's `ledger.json` (`candidates[].seat`,
`status: promoted`, `epochs[].promoted`). Without a ledger, the seat is the
candidate id with its `e<epoch>-` prefix removed, and the promoted candidate
is the first one whose mean equals the epoch's `improved.metric`. Trace lines
that do not decode are skipped, and so is a run with no trace.

### Timing

- `hive_ms`: from `epoch_started` to the epoch's first event after
  `observed`. That event is the first candidate being considered, and it is
  emitted as soon as the hive returns. The retreat is included.
- `scoring_ms`: from that event to `epoch_finished`. This covers the gates,
  scorers, decisions and promotion, with every seat idle. It is left unset
  while the epoch is running.

## Shape

```json
{"loop": "erdos-779",
 "runs": [{"run": "20261025-113628", "started_at": 1792928188, "maximize": true,
           "baseline": {"candidate": "a6c8d7dbb807", "mean": 0.1047, "failures": 0, "seeds": 2,
                        "promoted": false, "at_ms": 47},
           "epochs": [{"epoch": 1, "incumbent": "a6c8d7dbb807", "incumbent_metric": 0.1047,
                       "finished_metric": 0.1087, "hive_ms": 2706951, "scoring_ms": 988538,
                       "candidates": [{"candidate": "e1-main-b", "seat": "main-b", "mean": 0.1087,
                                       "failures": 0, "seeds": 2, "promoted": true, "at_ms": 3600000}]}]}],
 "upgrades": [{"id": "u-20261025-110000", "title": "Keep every seat working", "run": "20261025-113628",
               "before": {"run": "20261025-103058", "start": 0.103, "best": 0.1047, "end": 0.1047,
                          "candidates": 11, "promoted": 1},
               "after": {"run": "20261025-113628", "start": 0.1047, "best": 0.1087, "end": 0.1087,
                         "candidates": 11, "promoted": 1}}]}
```

An upgrade's `run` is its `applies_from.run`, or else the run whose `applied`
entry carried it. `before` summarizes the run before it and `after` the run it
applies to. An upgrade still waiting for its run has only `before`, which is
the latest run. One whose run is not in this store has neither.

## CLI

```text
rsi-ralph history <loop|name> [--csv | --json]
```

The text form lists each run with its baseline, then each epoch with its
incumbent, end score, seat and scoring minutes and its candidates, and then
each upgrade with the runs before and after it. The CSV has one row per
baseline or candidate:

```text
run,started_at,epoch,kind,candidate,seat,mean,failures,seeds,promoted,incumbent,incumbent_metric,seats_s,scoring_s,upgrades
```

`upgrades` lists the ids of the upgrades that apply from that row's run,
separated by `;`. It is empty when a run carries none.

## UI

`/api/loops/<loop>/history` serves the JSON. The run page's Score history
card draws one slot per run and epoch, left to right:

- each candidate is a dot, and promoted ones are larger and green;
- the incumbent each run started from is a line across its epochs;
- a dashed rule marks a run that carries upgrades (hover for ids and titles);
- under the chart, a table gives seat and scoring minutes per epoch.

## Invariants

- Reading the history never writes the store.
- A point's score is the `measured.mean` the run itself recorded. The
  history computes nothing new from the scorers.

## Follow-ups

- Merge runs of one loop that live in different run stores. The Erdős 779
  runs are split across `run8/`, `run9/` and `run10/`.
- Overlay an upgrade's predicted metrics on the graph. Today only the
  deciding score is plotted.
- Break scoring time down per candidate, from the `executed` durations.
