# Upgrade log: why a hive changed between runs

**Status:** Implemented. **Owner:** RSI Ralph maintainers.

Related: [RSI CLI](rsi-cli.md), [loop trace](loop-trace.md),
[proposal outcomes](proposal-outcomes.md).

## Problem

A loop gets better between runs as much as within them. After a run, an
operator (often working with an AI assistant) reads its measurements, finds a
root cause, edits the hive package or `rsi.json`, and starts the next run. The
reasoning behind those edits ends up in a hand-kept, free-form file (the Erdős
779 loop's `UPGRADES.md`) or nowhere. Nobody can then tell, from the run
store alone, why run 10's hive differs from run 9's, what the change was
supposed to do, or whether it did. An edit nobody wrote down is invisible.

## What changes a hive today

Checked against the code and the Erdős 779 run stores (runs 3 to 10):

- **Within a CLI run the hive is fixed.** `rsi-ralph run` parses the loop
  folder once (`crates/rsi-ralph-cli/src/cli/run.rs`), freezes it into
  `runs/<loop>/<run>/loop/` once (`store/mod.rs`, `create_run`), and hands
  the same in-memory package to every epoch (`hive/mod.rs`). Seats write only
  their candidate checkouts of the subject (`rsi-ralph-swarm/src/swarm/seat.rs`).
  `epoch-<n>/hive/` is the swarm's working folder (`events.jsonl`, `hive.db`,
  `runtime/`), not a copy of the package.
- **A seat cannot change its own hive.** None of the tools a seat holds
  (`hivemind_*`, the speech tools, the memory tools; see
  `vendor/tinyhivemind/crates/tinyhivemind-openhuman/src/tools/types.rs`)
  applies a patch or writes package files. The management tools change
  runtime membership only.
- **The constitution cannot be amended.** tinyhivemind's `Constitution`
  (`tinyhivemind-lang/src/memory/types.rs`) holds `max_turns`, `max_width`,
  `permitted_edits` and the like, but there is no amendment or ratification
  mechanism; `patch::guard` refuses any edit that changes it.
- **The retreat proposes subject ideas, not hive changes.** Its `{idea, why}`
  items go into the brief (`retreat/mod.rs`).
- **The CLI records no hive rationale.** Its `ideated` trace events say
  `"<seat> changed N files"` about subject candidates (`engine/mod.rs`).
- **The library loop does edit hives, and keeps the reason.** When a hive
  package is itself the subject (`rsi_ralph::orchestrator::run_proposal`),
  a `Proposal` carries a tinyhivemind `Patch`, a `title` and a `mechanism`;
  the archive stores both, and the lineage record's `rationale` is
  `"<title>: <mechanism>"`. tinyhivemind's lineage gives each design a
  content id, a parent and that rationale. That is a candidate being
  promoted, like a subject change in the CLI, and it already has a record.

So between CLI runs, a hive or loop config changes only when a person (or
their assistant) edits the loop folder. This spec records those edits,
detects the ones nobody recorded, and leaves a hook for the day a hive can
change itself.

## Goals and non-goals

- Goal: one record type for a change to a loop's hive package or config,
  saying who made it, what it rested on, the root cause, what changed, what
  it should do (measurably) and, later, whether it did.
- Goal: a record can carry several contributors, each with their own
  rationale, such as an assistant that diagnosed and an operator that
  approved.
- Goal: every run compares its loop folder with the previous run's and
  records any change no upgrade explains, so nothing changes silently.
- Goal: a Markdown rendering that reads like the hand-kept `UPGRADES.md`.
- Non-goal: letting a hive amend itself. Recording such amendments is
  specified (see [Hook](#hook-for-a-hive-that-changes-itself)) but nothing
  here adds the ability.
- Non-goal: the library loop's archive. Its proposals already keep their
  rationale; projecting them into this log is a follow-up.
- Non-goal: subject history. Promotion changes the subject by design and the
  run ledger records it.

## Where the log lives

The log is a set of `upgrades.jsonl` files, one JSON object per line. Their
union is the source of truth; the Markdown is only a rendering.

| File | Written by | Holds |
| --- | --- | --- |
| `<loop>/upgrades.jsonl` | `rsi-ralph upgrades add` and `outcome`, or by hand | upgrades and outcomes people record |
| `runs/<loop>/<run>/upgrades.jsonl` | the run, when it starts | its `applied` entry and any unexplained upgrade |
| `runs/<loop>/<run>/upgrades.md` | the run, when it starts | the whole log rendered, as of that start |
| `runs/<loop>/<run>/loop/upgrades.jsonl` | `create_run` (frozen copy) | the loop log as the run saw it |

The loop folder log travels with the loop and is frozen into every run, so a
run store alone shows which records each run carried. rsi-ralph never writes
the loop folder during a run.

## Records

Every line has a `kind`: `upgrade`, `outcome` or `applied`. Unknown fields
are ignored on read; optional fields are left out when empty.

### `upgrade`

```json
{"kind": "upgrade", "id": "u-20261025-110000", "recorded_at": 1792925000,
 "title": "Keep every seat working",
 "contributors": [
   {"author": {"kind": "assistant", "name": "claude-code", "model": "claude-opus-5-5"},
    "role": "diagnosed", "rationale": "The lead ends each seat's work at its first report."},
   {"author": {"kind": "operator", "name": "steven"},
    "role": "approved", "rationale": "Idle seats waste the epoch."}],
 "applies_from": {"run": "20261025-113628"},
 "evidence": [{"metric": "seats_busy_mean", "value": 7.2, "run": "erdos-779/20261025-103058"},
              {"note": "the scout and the skeptic stopped at about 4 minutes"},
              {"claim": "c-17"}],
 "diagnosis": "The lead prompt ended each seat's work at its first report.",
 "change": {"summary": "The lead answers every report with the next assignment.",
            "files": [{"path": "hive/seats/lead.md", "before": "<sha256>", "after": "<sha256>"},
                      {"path": "hive/constitution.json", "keys": ["max_turns"]}],
            "diff": null},
 "expected": [{"metric": "seats_busy_mean", "op": "ge", "value": 10, "note": "was 7.2"}]}
```

| Field | Meaning |
| --- | --- |
| `id` | Unique within the loop. `upgrades add` makes `u-<YYYYMMDD-HHMMSS>` (UTC), suffixed `-2`, `-3` on a clash. |
| `recorded_at` | Unix seconds. |
| `title` | One line. |
| `contributors` | At least one; see below. |
| `applies_from` | `run` and `epoch`. No `run` means the next run of the loop. |
| `evidence` | Each item has any of `metric` and `value`, `run` (a run id or `<loop>/<run>`), `claim` (a claim or ledger id) and `note` (the observation in words). |
| `diagnosis` | The root cause. |
| `change` | `summary`, `files` and an optional `diff` (a diff or diff summary). Each file has a `path` relative to the loop folder (ending in `/` for a folder), the `keys` touched in it, and `before`/`after` SHA-256 hashes (unset: absent or unknown). |
| `expected` | Measurable predictions: `metric`, `op` (`gt`, `ge`, `lt`, `le`, `eq`), `value`, optional `note`. |

**Contributors.** Each is `{author, role, rationale?}`. `author.kind` is:

- `operator` with `name`: the human running the loop;
- `assistant` with `name` and optional `model`: an AI assistant working with
  the operator, such as `claude-code` on `claude-opus-5-5`;
- `hive` with `seat`: a seat of the hive, the lead included;
- `loop` with `mechanism`: rsi-ralph itself, such as `run_start_diff`.

`role` is `authored` (the default), `diagnosed`, `proposed`, `approved`,
`applied` or `detected`. The `rationale` is that contributor's reason in their
own words. For an agent it is the text the agent gave, copied verbatim, never
a summary written afterwards.

### `outcome`

```json
{"kind": "outcome", "upgrade": "u-20261025-110000", "recorded_at": 1792932000,
 "run": "20261025-113628", "verdict": "mixed",
 "measured": [{"metric": "seats_busy_mean", "value": 10.4}, {"metric": "p90_s", "value": 45}],
 "contributors": [{"author": {"kind": "operator", "name": "steven"}, "role": "authored",
                   "rationale": "Busy, but slower."}]}
```

An outcome judges one upgrade from one run's measurements. `verdict` is
`confirmed` (every measured prediction held), `refuted` (none held) or `mixed`.
An upgrade may have several outcomes, one per run that measured it. Outcomes
are appended, never edited into the upgrade, so the log stays append-only.

### `applied`

```json
{"kind": "applied", "run": "20261025-113628", "recorded_at": 1792928188,
 "previous_run": "20261025-103058", "upgrades": ["u-20261025-110000"],
 "files": [{"path": "hive/seats/lead.md", "before": "…", "after": "…"}],
 "unexplained": []}
```

Written by every run when it starts: the previous run it compared with
(unset for the loop's first stored run), the upgrades it is the first to
carry, every file that differs with both hashes, and the changed paths no
upgrade names.

## The run-start diff

When `rsi-ralph run` has created the run folder, before the gateway starts:

1. The previous run is the loop's most recent stored run (by `started_at`)
   with a frozen `loop/` folder, in the same run store.
2. Every file of the loop folder (as `run` loaded it, `--env` merged into
   `rsi.json`) is compared with the previous run's frozen copy, except
   `upgrades.jsonl` and, for a `dir` subject, the subject's folder.
3. The run carries an upgrade when its loop log holds it and the previous
   run's frozen log did not (and its `applies_from.run` is unset), or when its
   `applies_from.run` names this run.
4. A changed file is explained when a carried upgrade's `change.files` names
   it or a folder above it (`hive/seats/`).
5. The run appends an `applied` entry to its own log. If anything is
   unexplained, it also appends an `upgrade` with id
   `u-<run>-unexplained`, title `Unexplained change since run <previous>`,
   one `loop`/`run_start_diff` contributor with role `detected`, the
   unexplained files with their hashes, and a `diff` summary (`added`,
   `removed`, or `modified (A -> B lines)` per file). It adds a warning to
   `run.json`: `changed since run <previous> with no upgrade record: <paths>`.
6. The run renders the whole log to its `upgrades.md`.

An unexplained entry is a prompt, not a failure: the run goes on. The
operator explains it afterwards with `upgrades add --run <run>`. Then
`upgrades show` lists both records.

## Hook for a hive that changes itself

`rsi_ralph_cli::upgrades::record(&RunDir, Upgrade)` appends an upgrade to a
run's log. It is the only writer for changes the loop or the hive makes to
itself. Today only the run-start diff calls it. Any later mechanism that lets
a seat amend the hive, its constitution or the loop config mid-run must call
it. The seat goes in as a `hive` contributor whose `rationale` is the reason
text the seat gave, verbatim, and `applies_from` names the run and epoch it
takes effect in.

## Markdown

`rsi-ralph upgrades show <loop>` renders the loop log followed by every stored
run's log, sorted by `recorded_at`:

```markdown
# Upgrades: erdos-779

## Keep every seat working

`u-20261025-110000` · from run 20261025-113628 · by assistant claude-code (claude-opus-5-5), diagnosed; operator steven, approved

**Saw**

- `seats_busy_mean` = 7.2 (run erdos-779/20261025-103058)

**Diagnosis.** The lead prompt ended each seat's work at its first report.

**Change.** The lead answers every report with the next assignment.

- `hive/seats/lead.md` (1a2b3c4d -> 5e6f7a8b)

**Why**

- assistant claude-code (claude-opus-5-5): The lead ends each seat's work at its first report.
- operator steven: Idle seats waste the epoch.

**Expected**

- `seats_busy_mean` >= 10 (was 7.2)

**Outcome in run 20261025-113628: confirmed** (by operator steven, authored)

- `seats_busy_mean` = 10.4: expected >= 10, held

## Run 20261025-113628 started

1 file changed since run 20261025-103058:

- `hive/seats/lead.md` (1a2b3c4d -> 5e6f7a8b)

Carries `u-20261025-110000`.
```

An upgrade's "from run" is its `applies_from.run`, or else the run whose
`applied` entry first carried it, or else "the next run".

## CLI

```text
rsi-ralph upgrades show    <loop|name> [--json]
rsi-ralph upgrades add     <loop> --title T --author SPEC [--rationale TEXT]... --change TEXT
                           [--saw TEXT]... [--metric M=V[@RUN]]... [--claim ID]...
                           [--diagnosis TEXT] [--file PATH[#KEY,KEY]]... [--expect "M OP V"]...
                           [--run RUN] [--epoch N] [--id ID]
rsi-ralph upgrades add     <loop> --from record.json
rsi-ralph upgrades outcome <loop> <upgrade-id> --run RUN [--metric M=V]... [--saw TEXT]...
                           [--verdict confirmed|refuted|mixed] [--author SPEC [--rationale TEXT]]...
```

- `show` takes a loop folder, or a loop name. With a name it reads the loop log
  from the latest stored run's frozen copy. `--json` prints the merged entries
  as JSON lines.
- `--author` is `KIND:NAME[@MODEL][=ROLE]`: `operator:steven=approved`,
  `assistant:claude-code@claude-opus-5-5=diagnosed`, `hive:lead=proposed`,
  `loop:run_start_diff`. Only an assistant takes `@MODEL`. The n-th
  `--rationale` belongs to the n-th `--author`, and a `--rationale` with no
  author is refused.
- `--file` hashes the file before (the latest run's frozen copy) and after
  (the loop folder now). A folder path ending in `/` is not hashed.
- `--from` reads a whole `upgrade` from a JSON file. It may leave out `id`
  and `recorded_at`, which suits an assistant that writes the record itself.
- `outcome` works out the verdict from the upgrade's predictions and the
  `--metric` values when `--verdict` is left out. It refuses when no
  prediction was measured.

Errors exit with status 2: a malformed `--author`, `--metric`, `--expect`
or `--file` (`invalid --author …`), a duplicate id
(`upgrade <id> already exists`), an unknown upgrade
(`unknown upgrade <id>`), and a log line that does not decode (named by line
number).

## UI

`serve` exposes `/api/runs/<loop>/<run>/files/upgrades.md`, and the run page
shows it in an Upgrades card.

## Invariants

- The log is append-only. An outcome never rewrites its upgrade.
- rsi-ralph never writes the loop folder during a run. Only `upgrades add` and
  `upgrades outcome` write `<loop>/upgrades.jsonl`.
- Every run started after this change has an `applied` entry. Every change to
  a loop file other than the log and a `dir` subject is either named by an
  upgrade the run carries or listed in an unexplained upgrade.
- A rationale is stored as its author gave it.

## Acceptance criteria

- A run whose loop folder equals the previous run's records an `applied`
  entry with no files and adds no warning.
- A run whose seat prompt changed with no upgrade naming it records the path
  as unexplained, writes the unexplained upgrade, and warns in `run.json`.
- The same change named by an upgrade added before the run is carried and
  explained.
- `upgrades show` renders an upgrade with an assistant and an operator, each
  with their own rationale, and its outcome.

## Converting a hand-kept `UPGRADES.md`

Add one `upgrade` per change bullet, with `upgrades add --from` or by hand:

- `title`: the bullet's bold lead-in.
- `contributors`: the operator, plus `assistant:<name>@<model>` where an
  assistant diagnosed the change.
- `applies_from.run`: the run id in the section heading ("Before run 10" is
  the id of run 10).
- `evidence`: the section's measured numbers as `metric`/`value`/`run`.
- `diagnosis`: the root cause sentence.
- `change.summary` and `files`.
- `expected`: what the change should do, written as a prediction where a
  number exists.

Then add one `outcome` per "Run N:" section, for the upgrades it measured.
Records written after the runs carry no `before`/`after` hashes unless they
are filled from the runs' frozen `loop/` copies.

## Follow-ups

- Compare across run stores: the previous run is found only in the same
  `$TINYRSI_HOME`.
- Project the library loop's archive (proposal `title`/`mechanism`, lineage
  `rationale`) into `upgrade` entries with a `hive` or `loop` author.
- Write `upgrade` records from a hive self-amendment mechanism, once one
  exists, through `record`.
- Store a full unified diff of unexplained changes, not just a summary.
