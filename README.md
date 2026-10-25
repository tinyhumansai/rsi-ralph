![Ralph](https://github.com/tinyhumansai/rsi-ralph/blob/main/docs/ralph.jpeg?raw=true)

# RSI Ralph

RSI Ralph keeps asking a team of AI agents to improve something until a score
says it is better. You give it a folder or git branch to improve, a way to
score it, and a short brief. A hive of OpenHuman agents edits a copy, your
scorer grades the copy against the current best, and the copy replaces the best
only when the numbers say it won. Then it goes again, until the goal is met or
the budget runs out.

## Ralph loops, goals, and what RSI Ralph adds

A **Ralph loop** is the simplest agent loop there is: hand an agent the same
prompt, let it work, then run it again on whatever it left behind, until it is
done. Nothing but the files carries over between runs, so progress has to land
in the work itself.

A **goal** is what turns that into something that can stop. Instead of looping
forever, you state a condition (tests pass, a score reaches a target) and keep
going until it holds. The agent, its prompt and its tools stay fixed; only the
subject of the work changes.

```text
   Ralph loop with a goal                  RSI Ralph

   same prompt, same agent, same tools     the prompt, agents, harness and tools
   edit the subject until the goal holds   are files too, and they get edited
   ┌───────────────────────────┐           ┌───────────────────────────────┐
   │ fixed:   prompt · agent   │           │ evolves: prompts · roles      │
   │          harness · tools  │           │          seats · policies     │
   │ changes: the subject      │           │          tools · context      │
   └───────────────────────────┘           │ and the subject, scored       │
                                           └───────────────────────────────┘
```

RSI Ralph keeps the loop and the goal and drops the assumption that the agent
is fixed. The team that does the editing is a tinyhivemind package of JSON and
Markdown: roles, seat prompts, shared context, the tools each seat may use,
and the policies that route turns between them. Mark those files editable and
they are part of what each epoch can change, next to the subject. A candidate
that edits its own team is scored like any other candidate, and it replaces the
current best only if it wins. The next epoch then runs the better team.

Two things stay out of reach so the loop cannot argue its way to a pass: your
scorer runs in your code, and the hive's `constitution.json` lists constraints
no edit may break. That is the recursive part of recursive self-improvement:
each generation of the team is the editor of the next. [How it improves
itself](#how-it-improves-itself) shows the loop, and the three depths of what
the agents may edit.

## Install

macOS and Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.ps1 | iex
```

This installs the `rsi-ralph` binary without `sudo`. Pinning a version, the
container image, building from source and upgrading are in
[INSTALL.md](INSTALL.md).

## Use cases

If you can score it with a script, you can point RSI Ralph at it. The loop works
on any folder or git branch where "better" has a number. Each card links to the
docs for the details.

<table>

<tr>

<td width="50%" valign="top">

<h3>Optimise an algorithm</h3>

<p>Point it at a heuristic and a scorer that counts what you care about: colours used, tour length, error. The agents rewrite the code, and a change is kept only if it beats the current best on the same seeds.</p>

<p>The starter loop does this: <a href="loops/graph-coloring">graph colouring</a></p>

</td>

<td width="50%" valign="top">

<h3>Speed up or shrink code</h3>

<p>Score runtime, memory or binary size with any script that prints <code>METRIC name=value</code>. Edits are limited to the files you mark editable, so the rest of the project stays untouched.</p>

<p><a href="docs/guides/rsi-cli.md">Writing a loop</a></p>

</td>

</tr>

<tr>

<td width="50%" valign="top">

<h3>Raise a pass rate</h3>

<p>Use a test suite or benchmark as the scorer. Held-out items stop the agents from fitting the examples they were shown.</p>

<p><a href="docs/rsi/scoring-acceptance.md">Scoring and acceptance</a></p>

</td>

<td width="50%" valign="top">

<h3>Tune prompts and rubrics</h3>

<p>Improve a prompt file, a rubric or a config against a judge. A Markdown rubric can act as an LLM judge, but only to hold a candidate back, never to accept one.</p>

<p><a href="docs/rsi/scoring-worked-example.md">Scoring worked example</a></p>

</td>

</tr>

<tr>

<td width="50%" valign="top">

<h3>Improve a hive</h3>

<p>Let the agents rewrite a tinyhivemind hive's seats, roles, prompts and policies. Changes go through private shadow evaluations and explicit approval, with rollback.</p>

<p><a href="docs/library.md">Library guide</a></p>

</td>

<td width="50%" valign="top">

<h3>Improve the improvers</h3>

<p>Mark the hive that does the editing as editable too. A better team writes better edits to the team, which is the recursive part of RSI.</p>

<p><a href="docs/architecture.md">Architecture</a></p>

</td>

</tr>

</table>

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

## A hive is defined by a language

The agents are not hard-coded. A hive is a *package* written in the
[tinyhivemind](https://github.com/tinyhumansai/tinyhivemind) language: plain
JSON and Markdown files that say who is on the team and how they behave. The
runtime reads the package and runs it, so changing the files changes the team.

```text
hive/
  hive.json           roles and seats: who exists, which tools each may use
  roles/lead.md       a role's charter
  seats/lead.md       the prompt a seat starts from
  context/rules.md    shared context every seat reads
  constitution.json   hard constraints no edit may break
  memory.json         what the hive remembers between epochs

        package (files)            tinyhivemind             OpenHuman
     ┌───────────────────┐     ┌─────────────────┐     ┌────────────────┐
     │ roles, seats,     │ ──▶ │ typed, checked  │ ──▶ │ agents run the │
     │ prompts, context, │     │ language: parse │     │ team: lead,    │
     │ policies, memory  │     │ and validate    │     │ implementers   │
     └───────────────────┘     └─────────────────┘     └────────────────┘
```

Because the whole team is text with a schema, it can be diffed, scored,
versioned and rolled back like any other source file. Every design is named by
a content ID, a SHA-256 over the package and its memory.

## How it improves itself

Each epoch is one turn of the loop. The agents edit a *candidate*, a copy of
the current best, and the scorer decides whether the copy replaces it.

```text
        ┌──────────────────────────────────────────────────────┐
        │                                                      │
        ▼                                                      │
   ┌─────────┐   ┌───────────┐   ┌─────────┐   ┌───────────┐   │
   │  brief  │──▶│  agents   │──▶│ scorer  │──▶│  paired   │   │
   │ history │   │ edit a    │   │ runs on │   │  test vs  │   │
   │ failures│   │ candidate │   │ both    │   │ the best  │   │
   └─────────┘   └───────────┘   └─────────┘   └─────┬─────┘   │
                                                     │         │
                                     won ────────────┤         │
                                     lost ───────────┤         │
                                                     ▼         │
                                     ┌───────────────────────┐ │
                                     │ keep the better copy; │─┘
                                     │ stop at goal or budget│
                                     └───────────────────────┘
```

What the agents may edit decides how deep the improvement goes:

```text
   subject only            subject + hive              the hive itself
  ┌─────────────┐        ┌────────────────┐        ┌──────────────────────┐
  │ agents fix  │        │ agents also    │        │ the agents' own      │
  │ the code    │   ──▶  │ rewrite their  │   ──▶  │ language files are   │
  │ being graded│        │ prompts, roles │        │ the thing graded, so │
  │             │        │ and context    │        │ the editors improve  │
  └─────────────┘        └────────────────┘        └──────────────────────┘
                                                      recursive self-improvement
```

In the last case the hive that edits is also the hive that gets edited. A
better team writes better edits to the team, and the next epoch starts from the
winner. The score stays outside the agents' reach: the scorer runs in your
code, the constitution lists what may never change, and edits outside the
editable paths fail the candidate before it is scored.

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

Completions: see [INSTALL.md](INSTALL.md#shell-completions).

Runs live in `~/.tinyrsi/runs/<loop>/<run-id>/`. Set `TINYRSI_HOME` or pass
`--home` to keep them somewhere else. Each run folder holds a frozen copy of
the loop, the ledger of every candidate and decision, the event log, and each
epoch's brief, diffs and step logs. Exit status is 0 on success, 1 when a run
fails, and 2 when the loop folder or arguments are invalid.

## Roadmap

The loop machinery is built and tested, and the CLI runs real loops end to end.
The [roadmap](ROADMAP.md) lists what has shipped and what comes next, and
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
