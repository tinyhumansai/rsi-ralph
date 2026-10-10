# OpenRouter integration run

Run on 2026-10-11 using `openai/gpt-4.1-mini` and an inherited
`OPENROUTER_API_KEY`. No credentials were read from `.env` or stored in artifacts.

```sh
TINYRSI_LIVE_OPENROUTER=1 TINYRSI_SMOKE_ARCHIVE=target/rsi-smoke/engine-grounded.json cargo run --offline -p tinyrsi --example live_openrouter
```

The actual pinned tinybackrooms engine ran the critic/proposer conversation,
formatter, repair and scoring. It made five model calls costing 5,249 measured
input/output tokens and returned one validated ranked context proposal.

| Stage | Paired seeds | Incumbent tokens | Candidate tokens |
|---|---:|---:|---:|
| Screen | 2 | 108 | 130 |
| Selection | 2 | 108 | 130 |
| Fresh confirmation | 2 | 108 | 130 |

Each task arm used the same execution cap and frozen output grammar. The
private deterministic arithmetic checker measured quality gain 1.000 with the
heuristic interval [1.000, 1.000]. The deliberately weak baseline forces a
constant answer. This result is a wiring test, not a scientific confidence
claim or a representative benchmark.

The approval port denied promotion. The selected baseline remained unchanged;
two lineage records and all three paired evaluation events survived reopening.
Earlier attempts rejected wrong-class, ungrounded, malformed and non-improving
proposals. The final smoke request restricts output to its public context class,
item, evidence references and edit bounds; it never shares private task draws
or oracle answers with the retreat. Real provider tool calls carry the exact
formatter/scorer schemas. Model responses can still fail validation, improve
nothing or exceed costs on another run; the example reports failure rather than
changing the acceptance rule.

This run did not activate a live hive, exercise a durable memory backend, run
Lean or code families, or estimate the retreat's incremental contribution.
Those obligations remain in [phase acceptance](phases.md).
