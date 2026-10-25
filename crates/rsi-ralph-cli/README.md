# rsi-ralph-cli

The `rsi-ralph` binary. It runs improvement loops defined as folders of JSON,
Markdown and small scripts, so a loop needs no Rust. Users start with
[`docs/guides/rsi-cli.md`](../../docs/guides/rsi-cli.md); the contract is
[`docs/specs/rsi-cli.md`](../../docs/specs/rsi-cli.md).

## Modules

| Module | Job |
| --- | --- |
| `manifest/` | Parse and validate a loop folder (`rsi.json`, brief, hive package, rubrics) from a path→text map |
| `keys/` | Resolve the key *names* a loop declares to values from the environment or `.env`; values never print |
| `store/` | `$TINYRSI_HOME`: run folders, `run.json`, atomic JSON, listing and interruption |
| `step/` | Run one gate or scorer command under the `RSI_*` step contract; read `result.json` or `METRIC` |
| `subject/` | `dir` copies or `git` worktrees for the incumbent and candidates; changes, diffs, promotion |
| `gate/` | The editable-surface check and the manifest's gate commands |
| `score/` | Run scorers per seed, grade with judges, decide by paired seeds or the sign test |
| `brief/` | Fill `{{incumbent}}`, `{{history}}`, `{{failures}}` and `{{ideas}}` |
| `retreat/` | The tinybackrooms creative retreat before each epoch, with memory across runs |
| `hive/` | The hive proposer: the loop's hive package run on OpenHuman agents via `rsi-ralph-swarm` |
| `engine/` | The epoch loop over the `Proposer`, `Ideator` and `Meter` ports; trace, ledger and run record |
| `upgrades/` | The upgrade log: records of why the hive or config changed, the run-start diff, the Markdown rendering |
| `serve/` | A std-only HTTP server with one embedded page over the store |
| `init/` | Bundled loop templates, compiled in from `loops/`, written by `rsi-ralph init` |
| `doctor/` | `rsi-ralph doctor`: programs on `PATH`, the model key's presence, the run store |
| `cli/` | `clap` commands and the wiring of keys, gateway, judges, hive and retreat |

## Design constraints

- The engine depends on ports (`Proposer`, `Ideator`, `Meter`), not on the
  agent runtime or the gateway, so it is tested offline with scripted stand-ins.
- Every model call — seats, judges, the retreat and any scorer that uses
  `$RSI_MODEL_URL` — goes through the run's `rsi-ralph-gateway`, which holds the
  only copy of the OpenRouter key and enforces the run and total caps.
- Stored documents are written atomically; secrets are scrubbed from stored
  errors.
- The UI is static HTML embedded with `include_str!`; building the binary
  never needs Node.
