![Ralph](https://github.com/tinyhumansai/rsi-ralph/blob/main/docs/ralph.jpeg?raw=true)

# RSI Ralph

RSI Ralph keeps asking a team of AI agents to improve something until a score
says it is better. You give it a folder or git branch to improve, a way to
score it, and a short brief. A hive of OpenHuman agents edits a copy, your
scorer grades the copy against the current best, and the copy replaces the best
only when the numbers say it won. Then it goes again, until the goal is met or
the budget runs out.

The name comes from the Ralph loop: run an agent on the same goal, over and
over, and let the results decide. The RSI part (recursive self-improvement) is
that the agents doing the editing can be the thing being improved.

## Install

macOS and Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.ps1 | iex
```

The installer picks the build for your machine from the latest
[release](https://github.com/tinyhumansai/rsi-ralph/releases), checks its
SHA-256, and puts `rsi-ralph` in `~/.local/bin` (on Windows,
`%LOCALAPPDATA%\Programs\rsi-ralph\bin`, which it adds to your user `PATH`).
It does not use `sudo`. To pin a version or choose the folder, pass flags
through the pipe:

```sh
curl -fsSL https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.sh \
  | sh -s -- --version v0.4.0 --dir /usr/local/bin
```

Prebuilt binaries exist for Linux (x86_64 and arm64, glibc 2.35+), macOS (Intel
and Apple silicon) and Windows (x86_64, which also runs on Windows on Arm). Run
the installer again to upgrade.

If you would rather not install anything, use the container image:

```sh
docker run --rm -it -v "$PWD:/work" -w /work -v rsi-ralph-home:/data \
  -e OPENROUTER_API_KEY ghcr.io/tinyhumansai/rsi-ralph doctor
```

Runs are kept in the `/data` volume. The UI only listens on loopback, so add
`--network host` to reach it from the host, or pass `--no-serve` to `run`.

To build from source you need Rust 1.96 or newer and the submodules:

```sh
git clone --recurse-submodules https://github.com/tinyhumansai/rsi-ralph
cd rsi-ralph && cargo install --path crates/rsi-ralph-cli
```

## Quick start

```sh
rsi-ralph doctor                    # git, diff, python3 and your API key
rsi-ralph init my-loop              # a starter loop: colour graphs with fewer colours
export OPENROUTER_API_KEY=sk-or-...
rsi-ralph check my-loop             # validate it before spending anything
rsi-ralph run my-loop               # run it, with a live UI on http://127.0.0.1:8787
```

The starter loop asks the agents to make a graph-colouring heuristic use fewer
colours. It scores with a small Python script, runs three epochs and stops at
$1 of model spend. When it finishes, the best version is in the run folder and
`rsi-ralph show` tells you where.

## What a loop is

A loop is a folder. You write JSON, Markdown and a scorer in whatever language
you like. You don't write any Rust.

```text
my-loop/
  rsi.json            what may change, how to score it, budget and epochs
  brief.md            the goal, written for the agents
  hive/               the agents: seats, roles and prompts
  scorers/colors.py   prints METRIC colors=<n>, or writes result.json
  subject/coloring.py the thing being improved
```

Each epoch, the lead agent reads the brief along with the history so far: past
scores, the failures your scorer reported, and ideas from an optional
brainstorming "retreat". Agents with a candidate checkout edit their copy.
Edits outside the paths `rsi.json` marks editable fail the candidate before it
is scored. Your scorer then runs on the candidate and on the current best, on
the same seeds or held-out items, and a paired test decides which one wins.
Markdown rubrics can act as LLM judges too, but a judge on its own can only
hold a candidate back, never accept one.

Every model call goes through a local gateway that holds the only copy of your
key and stops the run at its spend cap. Key values are never written to the
run store.

[Writing a loop](docs/guides/rsi-cli.md) walks through every field of the
starter loop. [The CLI spec](docs/specs/rsi-cli.md) is the full contract.

## Commands

| Command | What it does |
| --- | --- |
| `rsi-ralph init <dir>` | Write a starter loop into a new folder (`--list` shows the templates) |
| `rsi-ralph doctor` | Check that `git`, `diff`, `python3` and `OPENROUTER_API_KEY` are available |
| `rsi-ralph check <loop>` | Validate a loop folder and report missing keys |
| `rsi-ralph run <loop>` | Run it, serving the UI (`--epochs N`, `--no-serve`, `--env K=V`) |
| `rsi-ralph score <loop>` | Score one checkout with the loop's scorers, outside a run |
| `rsi-ralph runs` | List stored runs, newest first |
| `rsi-ralph show <loop>/<run>` | One run's outcome, spend, errors and folder |
| `rsi-ralph serve` | The UI over every stored run |
| `rsi-ralph completions <shell>` | Print a completion script for bash, zsh, fish, PowerShell or elvish |

To turn on completions in zsh, for example, add
`source <(rsi-ralph completions zsh)` to `~/.zshrc`.

Runs live in `~/.tinyrsi/runs/<loop>/<run-id>/`. Set `TINYRSI_HOME` or pass
`--home` to keep them somewhere else. Each run folder holds a frozen copy of
the loop, the ledger of every candidate and decision, the event log, and each
epoch's brief, diffs and step logs. Exit status is 0 on success, 1 when a run
fails, and 2 when the loop folder or arguments are invalid.

## Where it stands

The loop machinery is built and tested, and the CLI runs real loops end to
end. Whether it produces genuine recursive improvement is still an open
question. The one complete preregistered experiment, 24 attempts comparing an
original and an evolved hive, came out inconclusive. The examples in this
repository are toy problems and do not show positive RSI. The
[roadmap](ROADMAP.md) lists what has shipped and what comes next, and
[docs/rsi/](docs/rsi/README.md) has the research notes and dated results.

## Using it as a library

Everything the binary does is also available as Rust crates, and the library
has more besides. There is a host runtime that improves tinyhivemind hives
through typed patches, private shadow evaluations and explicit human approval,
with durable epochs, deployment and rollback. There is also a lighter loop that
improves any script that prints `METRIC name=value`.
[docs/library.md](docs/library.md) describes both, with the runnable examples.

## Contributing

[docs/development.md](docs/development.md) covers the build, tests, live runs
and releases. [AGENTS.md](AGENTS.md) is the working agreement for humans and
coding agents alike, and [docs/README.md](docs/README.md) indexes the rest of
the documentation. Security reports go through [SECURITY.md](SECURITY.md).

RSI Ralph is licensed under the [GPL-3.0](LICENSE).
