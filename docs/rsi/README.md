# TinyRSI research and implementation

The improvement object is a typed tinyhivemind hive package and its pinned
memory snapshot. An external host proposes, shadow-evaluates and requests
human approval; private verifiers decide which design is selected.

- [Framework](specs/rsi-framework.md): epoch, ownership and phase boundaries.
- [Phase status and acceptance](phases.md): implemented components, outstanding
  host integrations and experimental follow-ups for P1–P6.
- [Verified decomposition](specs/way-of-working.md): runtime blueprint and
  host-observed verification.
- [Scoring](specs/scoring.md): eval-owned checkpoints, family normalization,
  bootstrap uncertainty and anchor progress.
- [Implementation contract](specs/orchestrator.md): durable lineage and gates.
- [Survey](rsi-survey.md): literature and citation-verification status.
- [OpenHuman substrate](openhuman-substrate.md): earlier integration research.
- [Implementation plan](../plans/2026-10-10-rsi-orchestrator.md).

The live OpenRouter arithmetic example validates wiring. It does not establish
research-benchmark improvements or recursive ignition.

See [the measured OpenRouter smoke run](live-smoke.md) for integration evidence.
