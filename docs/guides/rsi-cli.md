# Writing a loop for the `rsi-ralph` binary

A loop is a folder. `rsi-ralph` runs it: it gives a hive of agents the brief, lets
them edit candidate copies of your subject, scores each candidate with your
score functions, keeps the ones that are better, and records everything. You
write no Rust. The full contract is [`../specs/rsi-cli.md`](../specs/rsi-cli.md);
this guide walks through [`loops/graph-coloring`](../../loops/graph-coloring).

```sh
curl -fsSL https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.sh | sh
rsi-ralph doctor                             # git, diff, python3, OPENROUTER_API_KEY
rsi-ralph init graph-coloring                # this guide's loop, as a folder of your own
export OPENROUTER_API_KEY=...                # or put it in <loop>/.env
rsi-ralph check graph-coloring
rsi-ralph run   graph-coloring               # UI on http://127.0.0.1:8787
```

On Windows install with
`irm https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.ps1 | iex`;
from a checkout, `cargo install --path crates/rsi-ralph-cli` builds it instead.
`rsi-ralph init --list` names the bundled templates.

## The folder

```text
loops/graph-coloring/
  rsi.json            what to improve, how to score it, limits
  brief.md            the goal, in prose, for the hive
  hive/               who does the work (tinyhivemind hive language)
  scorers/colors.py   a score function
  subject/coloring.py the thing being improved
```

## 1. Say what may change: `subject`

```json
"subject": { "kind": "dir", "path": "subject", "editable": ["coloring.py"] }
```

A `dir` subject is copied for each candidate; your folder is never touched and
the best version ends up in the run folder (`incumbent/`). For code in a git
repository use `"kind": "git"` with a `path` (absolute, or relative to the
loop folder and free to leave it with `..`) and a `branch`: every
candidate is a branch and worktree, and promotion fast-forwards `branch`.
`editable` and `frozen` are path prefixes; anything outside them fails the
candidate before it is scored.

## 2. Say what "better" means: `score`

A score function is any command. It runs in the loop folder with
`$RSI_CANDIDATE` pointing at the checkout to judge, and either prints
`METRIC <name>=<number>` or writes `$RSI_OUT/result.json`:

```json
{ "score": 0.62,
  "splits": { "test": { "items": { "q17": 1, "q18": 0 } } },
  "failures": [ { "id": "q3", "type": "temporal", "text": "what went wrong" } ] }
```

`failures` and `metrics` are shown to the hive in the next brief; put anything
the hive must not see, such as held-out accuracy, in a `"private": {...}` map
instead. A score function that
calls models itself (a reader, a judge, an embedder) should send them through
the run's gateway so they count against the budget: `$RSI_GATEWAY/__tag/<phase>/<name>/v1`
is an OpenAI-compatible base URL that needs no key. Pick how a candidate wins:

- per-seed scores, compared with a paired test — `"test": "seeds"`, with
  `"seeds": [1, 2, 3]` (each seed reaches the scorer as `$RSI_SEED`);
- per-item pass/fail on a held-out split, with an exact sign test —
  `"test": "sign", "split": "test"`.

Seeds run one after another. When every seed is slow and independent, such as
a scorer that boots a game server and plays a ten-minute episode, set
`"parallel": 3` beside `scorers` and `decide` in `score` to run up to three
seeds of a scorer at once. Each seed keeps its own `$RSI_OUT`, timeout and
output cap, and results stay in seed order; make sure seeds do not fight over
shared resources such as ports (derive them from `$RSI_SEED`).

A Markdown rubric becomes an LLM judge: `{ "id": "clarity", "judge":
"scorers/clarity.md", "weight": 0.2 }`. Judges grade the candidate's diff; they
can hold a candidate back but never accept one on their own.

## 3. Say who does the work: `hive`

The hive is a [tinyhivemind hive package](../../vendor/tinyhivemind/docs/specs/hive-language.md):
`hive.json`, `constitution.json`, `memory.json` and Markdown prompts under
`seats/`, `roles/` and `context/`. `rsi.json` names the `lead` seat, which
receives the brief. A seat whose `config.action_dir` is `"{candidate}"` gets its
own candidate checkout to edit; the others see the incumbent read-only.
Every section with a safe default may be left out (see the hive language's
"Optional fields and defaults"), so a hive is mostly its seats and prompts:
[`loops/graph-coloring/hive`](../../loops/graph-coloring/hive) is a complete
two-seat example. Multi-seat hives should widen `coordinator.round_width`
(and `policies.routing`/`division`), whose defaults of 1 run one turn at a
time. rsi-ralph adds the hive tools and the turn-taking rules itself.

## 4. Tune creativity, models, keys and money

```json
"models": { "default": "z-ai/glm-5.3-flash" },
"creativity": {
  "temperature": 0.4,
  "retreat": { "variations": 3, "turns": 6, "ideas": 5, "mode": "critic_pair", "temperature": 1.1 }
},
"keys": { "web_search": { "provider": "gemini", "env": "GEMINI_API_KEY", "seats": ["lead"] } },
"budget": { "run_usd": 5, "total_usd": 50, "score_reserve_usd": 0.6 }
```

- The **retreat** runs tinybackrooms conversations over the brief before each
  epoch and puts the best ideas into `{{ideas}}`; with `memory` it remembers
  which ideas led to improvements across runs.
- **Keys** name environment variables, never values. `web_search` gives the
  listed seats (all, when `seats` is omitted) OpenHuman's web search tools
  with that provider (`brave`, `exa`, `gemini`, `parallel`, `querit`,
  `tavily` or `tinyfish`); the key is pinned in each seat's memory only. They are read from the
  environment or `<loop>/.env`, and never written to the run store.
- Every model call goes through a **spend gateway** that stops the run at
  `run_usd` or `total_usd`. Models other than the built-in GLM 5.3 Flash need a
  price ceiling in `budget.prices`.

## 5. Watch it

`rsi-ralph run` serves the UI while it works: the score chart, every candidate
with its decision and diff, each epoch's brief, and the live event log.
Afterwards:

```sh
rsi-ralph runs                         # every run, newest first
rsi-ralph show graph-coloring/<run-id> # one run's outcome, error, warnings and folder
rsi-ralph serve                        # the UI over all stored runs
```

If the model provider refuses every call of an epoch (a key over its spending
limit, say), the run stops as `failed` with reason `provider_refused`, `rsi-ralph run`
exits with status 1, and `show` prints the provider's status and message on its `error` line. Partial
refusals in an epoch that changed nothing appear as `warning` lines and the
run goes on.

Runs live in `~/.tinyrsi/runs/<loop>/<run-id>/` (set `TINYRSI_HOME` to move
them): a frozen copy of the loop, `run.json`, `ledger.json`, `events.jsonl`,
and per-epoch briefs, candidates and step logs.
