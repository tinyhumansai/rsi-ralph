# Script loop

A complete observe, ideate, execute, measure, score, improve loop over a plain
script, supervised by a language model. Behavior is specified in
[`docs/specs/script-loop.md`](../../../../docs/specs/script-loop.md); the
user-facing guide is [`docs/guides/script-loop.md`](../../../../docs/guides/script-loop.md).

## Public surface

- `run_script_loop(subject, supervisor, policy, run, trace) -> Result<LoopReport>`
- `ScriptSubject`, `ScriptCommand`, `MetricSpec`: what to run and measure.
- `LoopPolicy`: epochs, proposals per epoch, patience, token budget, how many
  rejected proposals the supervisor sees, and the paired `EvalPolicy`.
- `Supervisor` and `SupervisorLimits`: the model side.
- `ScriptProposal`, `ProposedFile`: the forced tool call's schema;
  `ScriptProposal::validate` enforces the editable set.
- `ScriptLedger`, `LedgerCandidate`, `SeedRun`, `IncumbentEntry`,
  `CandidateDecision`: the `ledger.json` snapshot.
- `parse_metric`: the `METRIC <name>=<value>` contract.

## Files

| File | Role |
| --- | --- |
| `mod.rs` | the loop: baseline, epochs, promotion, stop reasons, ledger writes |
| `types.rs` | subject, policy, proposal, ledger and report types |
| `execute.rs` | validation, content ids, materialization, per-seed runs, metric parsing |
| `score.rs` | the quality transform and paired scoring over `evaluation::compare` |
| `supervisor.rs` | prompt, tool schema, forced tool call, a thread-parking executor |
| `test.rs`, `test_loop.rs` | unit tests and end-to-end loop tests |

## Run directory

```text
<run>/
├── events.jsonl          # when the caller passes a JsonlTrace here
├── ledger.json           # rewritten atomically after every candidate
├── candidates/<id>/      # workspace copy + candidate sources, one per content id
└── tmp/                  # per-seed capture and TMPDIR, removed after each seed
```

## Design notes

- **Content ids** are the first 12 hex digits of SHA-256 over the sorted
  editable paths and contents (length-prefixed). Equal content is never run
  twice; a repeat is recorded as `duplicate`.
- **Execution** reuses the crate's private `process` runner, the same one
  host verification uses: own process group, cleared environment, stdin
  closed, group kill on timeout or output overflow. The child environment is
  exactly `PATH=/usr/bin:/bin`, `TMPDIR`, the subject's `env` entries, and
  `TINYRSI_SEED`.
- **Scoring** maps each seed's metric to `q = 1/2 + atan(s * m) / pi` so
  `evaluation::compare` (which needs qualities in `[0, 1]`) sees the same
  order as the metric. The default policy uses margin 0: accept when the
  paired lower bound is above zero. Very large metrics compress under
  `atan`; rescale a metric reported in the millions before relying on it.
- **Supervisor futures** are driven on the loop's thread, so a `ModelPort`
  must not depend on a particular async runtime. `OpenRouterModel` runs its
  request on a worker thread and is compatible.
- **Failures:** one bad seed fails only that candidate. Invalid or malformed
  proposals are recorded and the loop continues. Model transport failures,
  trace or ledger I/O failures and a failing baseline end the run with
  `run_finished { reason: "error" }`.

## Safety

Proposals are code. They run locally with your privileges, on a copy of the
workspace, with a cleared environment, a timeout and an output cap — and
nothing else. There is no network or filesystem sandbox. Run live supervision
in a container or VM when that matters.
