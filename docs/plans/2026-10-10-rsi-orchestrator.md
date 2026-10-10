# RSI orchestrator implementation

Spec: ../rsi/specs/orchestrator.md and ../rsi/specs/way-of-working.md.

1. Pin tinyhivemind; retain the legacy wire API, re-export its exact Record.
2. Evaluation module: write failing paired-data and decision tests, implement
   typed seeds/budgets/outcomes, multi-seed confidence and hard constraints.
3. Archive module: write failing restart, duplicate, ancestry and rollback tests,
   implement exclusive atomic persistence of exact language records/packages
   and proposal outcome events.
4. Observation and blueprint modules: test cursor replay, DAG readiness and
   verified completion; implement read-only adapter and deterministic dossier.
5. Orchestrator module: test guard/dedup/evidence/eval/confirmation/approval
   rejection, implement staged host ports and durable record-before-activation.
6. OpenRouter example: typed proposal, paired fresh tasks/seeds, deterministic
   private checker, archive restart and approval rejection. Opt-in credentials
   come from environment only.
7. Run fmt, clippy, build, tests and rustdoc, inspect coverage; run the live
   example with local OpenRouter credentials. Document measured results and
   remaining external phases without claiming research experiments complete.
