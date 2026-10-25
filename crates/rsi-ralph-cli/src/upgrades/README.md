# `upgrades/`: the upgrade log

Records why a loop's hive package or config changed between runs, detects
changes nobody recorded, and renders the log as Markdown. The contract is
[`docs/specs/upgrade-log.md`](../../../../docs/specs/upgrade-log.md).

| File | Job |
| --- | --- |
| `mod.rs` | Read, parse and append `upgrades.jsonl`; `collect` a loop's log across runs; `record` (the hook for self-changes); ids, hashes, `evaluate` |
| `types.rs` | `Entry` (`upgrade`, `outcome`, `applied`) and its parts, the stored form |
| `drift.rs` | The run-start diff: compare with the previous run's frozen loop, write `applied` and any unexplained upgrade, render `upgrades.md` |
| `render.rs` | The Markdown rendering |
| `notation.rs` | The command-line forms: `KIND:NAME[@MODEL][=ROLE]`, `METRIC=VALUE[@RUN]`, `METRIC OP VALUE`, `PATH[#KEY,KEY]` |

Constraints:

- The log is append-only. Outcomes are new lines, never edits.
- A run writes only its own `runs/<loop>/<run>/upgrades.jsonl` and
  `upgrades.md`. Only `rsi-ralph upgrades add` and `outcome` write the loop
  folder's log.
- The run-start diff runs before the gateway starts and needs no network.
- A run started before this module existed has no log. `collect` treats a
  missing file as empty.
