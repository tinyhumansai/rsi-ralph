# Research: Recursive Self-Improvement (RSI)

Collected groundwork for building an RSI framework on OpenHuman agents.
Status: early research, nothing here is a commitment.

## Files

- `rsi-survey.md` — the landscape: the Weco RSI paper (deep dive) plus
  Darwin Gödel Machine, GEPA, Gödel Agent, and friends.
- `openhuman-substrate.md` — how OH's configurable components (tools,
  workflows, memory, skills, prompts, sub-agents) map onto an evolvable
  environment for the loop.
- `specs/rsi-framework.md` — the working spec: a two-loop RSI system where
  the improvement object is the OH agent's own harness configuration.

## Core idea

RSI = pointing the improvement loop at the improver. Every accepted rewrite
becomes the thing the next round edits. The Weco result (2026) shows this is
empirically viable at the *harness layer*: prompts, search policy, context
management, selection rules. Not weights. That is exactly the layer OH exposes
as user-configurable components, which is why OH is a credible host for this.

## Reading list

- Weco RSI paper: https://arxiv.org/html/2609.26457v1 (primary)
- Darwin Gödel Machine: https://arxiv.org/abs/2505.22954
- GEPA (reflective prompt evolution): https://arxiv.org/abs/2507.19457
- Gödel Agent (self-referential framework): https://arxiv.org/abs/2410.04444
- Mendel Gödel Machine: https://arxiv.org/html/2608.07645
- AIDE (inner-loop ancestor): https://arxiv.org/abs/2502.13138
