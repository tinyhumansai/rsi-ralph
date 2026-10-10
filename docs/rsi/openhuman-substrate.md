# OpenHuman as an RSI Substrate

How OH's configurable components map onto the pieces an RSI loop needs.
Grounded in OH's actual exposed surface (skills, workflows, memory, tools,
prompts, scheduling, sub-agents) — anything marked *verify* needs a check
against the running system before we build on it.

## The evolvable environment

The user's framing: define a **generic environment that can be evolved over
time**. In RSI terms the "environment" is the agent's own harness — the
configuration that turns a fixed model into a working agent. OH already
decomposes that harness into named, mutable surfaces:

| OH component | Role in the environment | Mutability |
|---|---|---|
| Prompts / agent definitions (incl. sub-agent TOML + system Markdown) | The harness text the paper's loop optimized most profitably | Direct mutation target, cheap to diff and revert |
| Skills (Markdown capability modules, registry) | Capabilities the agent can pull in; add/remove/tune | Evolvable capability set |
| Workflows (tinyflows DAGs, checkpoints, triggers) | Structured multi-step behavior; automatable eval harnesses | Evolvable behavior; also the *substrate that runs the loop itself* |
| Memory (memory trees, budgets, beliefs, compaction rules) | Context management — the paper's discovered improvements live exactly here | Evolvable policy: budgets, injection rules, compaction thresholds |
| Tools + tool routing (classifier, permissions, approval gates) | Action space; also a safety surface | Evolve conservatively; approval gates are a guardrail, not a target |
| Model routing / pinning per sub-agent | Compute allocation per role | Evolvable like the paper's inner/outer model split |
| Scheduling / background jobs | Lets the loop run autonomously on a cadence | Infrastructure |
| Integrations (MCP, OAuth toolkits) | New task domains plug in as data | Extends the task families without code changes |

Key insight: the paper's seven accepted rewrites are each expressible as an
OH configuration change — search policy (workflow/sub-agent logic), bounded
prompts (prompt surface), failure memory (memory injection rule), robustness
safeguards (prompt text), selection penalties (workflow logic). Nothing the
loop found required touching model weights or OH's core code. The environment
boundary is exactly the config layer.

## Mapping the two-loop architecture onto OH

- **Inner-loop agent** = an OH sub-agent (e.g. `Coder`-like role) operating
  on a task codebase under a fixed budget. Its behavior is defined by config
  (prompt + skills + memory policy + model pin).
- **Outer-loop agent** = a stronger OH sub-agent with tools to read the
  incumbent's config, the grading history (memory tree), and propose
  rewrites as config patches.
- **Grading** = a tinyflows workflow: run candidate config on the selection
  tasks (scheduled/background jobs fan out runs), collect scores on held-out
  data, aggregate, append to the lineage record in memory.
- **Lineage/archive** = a memory subtree: one record per candidate config
  (config snapshot hash, grade, per-task scores, diffs, verdict).
- **Acceptance** = workflow step applying the paper's rule (private grade
  improves incumbent's) with a margin; reject and log otherwise.
- **Human in the loop** = OH approval gates. First iterations should require
  explicit human approval before any accepted rewrite goes live; the loop can
  still run fully autonomous in shadow mode, only *proposing*.

## Guardrails specific to OH

- Approval gates, filesystem permissions, and secret handling must be
  **frozen** — outside the mutable surface. An RSI loop editing its own
  guardrails is the failure mode we exist to avoid.
- Config mutations should be versioned snapshots (content-addressed), never
  in-place edits, so any accepted state is revertible.
- The grading workflow must run candidates with the *candidate's* config in
  an isolated context, not the live agent's — shadow evaluation.
- Budget ceilings per evaluation are infrastructure, not evolvable.

## Open questions

- *verify*: exact snapshot/restore mechanics for OH config (is config
  exportable/importable wholesale today? which parts live in the DB vs
  files?).
- *verify*: whether sub-agent definitions and prompts are addressable by a
  stable ID for diffing.
- How to construct a diverse selection benchmark for *general assistant*
  work (the paper used AI R&D tasks; ours depends on what we want the OH
  agent to get better at).
- Private held-out sets per task family: who curates them, and how do we
  keep them out of the loop's view (OH memory hygiene)?
