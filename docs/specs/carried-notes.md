# Carried notes

**Status:** Implemented. **Owner:** RSI Ralph maintainers.

Related: [RSI CLI](rsi-cli.md), [score history](score-history.md),
[upgrade log](upgrade-log.md).

## Problem

In a `run`, each editing seat works in its own checkout of the subject. At
the end of an epoch one candidate is promoted, and every seat of the next
epoch starts from that checkout. Whatever the other seats wrote is gone from
the loop, their notes included.

The Erdős 779 loop asks every seat to keep `notes/<seat>.md`: what it
checked, what is still open, the obstacle and its next steps. In run
`20261025-113628`, every `epoch-1/candidates/<seat>/notes/` holds 10 to 30
KB of real notes. In `epoch-2/candidates/`, five seats (`dens-a`, `dens-b`,
`refute-a`, `refute-b`, `chen-b`) hold exactly the promoted `reducer`'s
11,014 bytes of notes. None of their own epoch-1 notes reached epoch 2.
`{{incumbent}}`, `{{history}}` and `{{failures}}` describe scores and
decisions, and none of them carries what a losing seat found.

## Goals and non-goals

- Goal: every candidate's notes, promoted or not, reach the next epoch's
  brief, one section per seat, with the seat's candidate and outcome.
- Goal: a copy of each epoch's notes in the run store, which stays readable
  after the run and is shown in the `serve` UI.
- Goal: caps per file, per epoch and per brief, so that one seat cannot
  flood the store or the prompt.
- Non-goal: carrying notes across runs. Hive memory (`hive.memory`) does
  that.
- Non-goal: writing other seats' notes into the next checkout; see
  [Why the brief and not the checkout](#why-the-brief-and-not-the-checkout).

## `rsi.json`: `carry`

The feature is opt-in: without `carry`, nothing is harvested and briefs are
unchanged.

```json
"carry": {
  "notes": ["notes/"],
  "file_bytes": 65536,
  "epoch_bytes": 524288,
  "brief_chars": 24000
}
```

| Field | Default | Meaning |
| --- | --- | --- |
| `notes` | required | Path prefixes in the subject whose changed files are carried, matched like `subject.editable` (`notes/` and `notes` both cover `notes/a.md`). At least one; each a plain relative path. |
| `file_bytes` | 65536 | Most bytes of one file kept in the store; the rest is cut on a character boundary. |
| `epoch_bytes` | 524288 | Most bytes kept across every seat of one epoch. |
| `brief_chars` | 24000 | Most characters of notes text in one brief. Headings are extra. |

Unknown fields fail, and a zero cap fails `check` with
`invalid carry.<field>: must be positive`. A carry prefix has to be under
`subject.editable` for a seat to change it without failing the surface
gate. `check` does not enforce that, because a prefix outside the
surface simply carries nothing.

## Harvest

When an epoch has finished promoting, or has decided not to, the engine
looks at each of its candidates. It takes the paths in the candidate's
`changed` list (the same list the surface gate checks) that sit under a
`carry.notes` prefix. Paths that no longer exist, because the seat deleted
them, are skipped, and so are files that are not UTF-8 text.

Only changed files count. A seat that left the incumbent's notes alone has
nothing of its own to carry. Without this rule, in run `20261025-113628`
the reducer's note would have been harvested again under every seat that
started from it.

Then the caps apply, in order:

1. Each file is cut to `file_bytes`.
2. `epoch_bytes` is water-filled across seats, using each seat's total after
   step 1. Seats are served smallest first. Each one gets either what it
   needs or an even share of what is left. So a seat with a short note
   keeps all of it, whatever a verbose seat wrote. Within its share a seat
   keeps its files in path order. The file that crosses the share is cut,
   and any files after it keep 0 bytes. They are still listed, so the cut
   is visible.

### Store layout

```text
runs/<loop>/<run-id>/epoch-<n>/
  notes.json                 the index (below); written whenever carry is set
  notes/<seat>/<path>        each kept file, e.g. notes/dens-a/notes/dens-a.md
```

`notes.json`:

```json
{"epoch": 1,
 "promoted": "e1-reducer",
 "seats": [{"seat": "dens-a", "candidate": "e1-dens-a", "outcome": "rejected",
            "files": [{"path": "notes/dens-a.md", "bytes": 21034, "kept": 21034}]}]}
```

`promoted` is the candidate the epoch promoted, or `null`. `seats` lists,
by seat id, only the seats that changed at least one carried file. An
epoch where no seat did writes `"seats": []`. `outcome` is the word
`{{history}}` uses for the candidate: `promoted`, `accepted but not best`,
`rejected`, `inconclusive`, `gated`, `unscored` or `error`. `bytes` is the
file's size in the checkout and `kept` what the store holds: less when a
cap cut it, 0 when `epoch_bytes` left no room.

## Brief: `{{notes}}`

`{{notes}}` renders the previous epoch's harvest. In epoch 1, and whenever
the previous epoch harvested nothing, it reads `None yet.`, like every
other placeholder. A brief that leaves it out gets a
`## Notes from the last epoch` section appended when there are notes, as
the other placeholders do.

```markdown
The notes each seat changed in epoch 1. Only the candidate epoch 1 promoted
`e1-reducer` reached your checkout; the other seats' notes are only here.

### `dens-a` (`e1-dens-a`, rejected)

#### `notes/dens-a.md`

…the note…

(cut to 65536 of 70112 bytes)

### `reducer` (`e1-reducer`, promoted)
…
```

When the epoch promoted nothing, the first paragraph says so instead
(`Since epoch 1 promoted no candidate, …`). `brief_chars` is water-filled
over the seats' sections in the same way as `epoch_bytes`. A section that
does not fit is cut with `…`, and a code fence the cut leaves open is
closed, as `{{failures}}` does. A file that the store itself cut ends with
`(cut to <kept> of <bytes> bytes)`. A file that `epoch_bytes` dropped reads
`(dropped: the epoch's notes cap was reached)`.

## Why the brief and not the checkout

The other option is to write the harvest into the next incumbent, for
example under `notes/epoch-<n>/<seat>.md`, so seats could open the notes as
files. This change does not do that, for three reasons:

- **Scores.** A scorer that reads `notes/*.md`, such as the Erdős 779 judge,
  would credit every next-epoch candidate with every other seat's findings.
  The incumbent's own score was taken without them, so each candidate would
  start from an inflated score.
- **`git` subjects.** Candidates are worktrees branched from the incumbent's
  branch, not copies of its folder. Files placed in the incumbent worktree
  would not reach them unless the run committed to the user's branch.
- **Overwrites.** A carry folder that a seat may edit is part of what gets
  promoted, so a winning candidate could rewrite another seat's carried
  notes. The store copy is written once and never promoted.

The brief reaches every seat without these problems. The full text stays in
the store for the UI and for any step that reads `$RSI_RUN_DIR`.

## UI

`/api/runs/<loop>/<run>/files/epoch-<n>/notes.json` and
`/api/runs/<loop>/<run>/files/epoch-<n>/notes/<seat>/<path>` serve the
harvest. Every segment must be a plain name. The run page has a Notes card
with one tab per epoch. Each tab shows that epoch's promoted candidate and
then each seat with its outcome and its files, with their cuts marked.

## Invariants

- A harvested file is never promoted, and no later epoch rewrites it.
- The kept bytes of an epoch never exceed `epoch_bytes`, and no file's kept
  bytes exceed `file_bytes`.
- Without `carry`, the engine writes nothing new and renders the same brief
  as before.

## Acceptance criteria

- In a two-epoch run with `carry`, the notes of a seat that was not
  promoted appear in epoch 2's brief, under its seat, candidate and outcome.
  The brief also names the promoted candidate.
- A seat that changed no carried file contributes no section. A note that
  is unchanged from the incumbent is not harvested again.
- `rsi.json` without `carry` serializes exactly as before.

## Follow-ups

- An opt-in carry folder in the checkout for `dir` subjects, for loops whose
  scorers should see every seat's notes. It would have to be excluded from
  `changed` and from the surface gate.
- Notes from more than the previous epoch, for instance a rolling summary.
