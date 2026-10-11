# P1 offline evidence demonstration — 2026-10-11

`cargo run -p tinyrsi --features host --example observe_epochs` completed two
actual coordinator turns with independently observed golden checker passes.
The deterministic offline provider reported ten input and two output tokens per
turn. Those counters are adapter measurements, not billed hosted-model tokens.

The example reopened both SQLite runtime and evidence journal between epochs.
Repeating each checkpoint returned the original receipt without consuming later
evidence. A second invocation reopened the completed run without dispatching new
work. The [published receipts](results/2026-10-11/p1-epochs.json) contain only
identities, transcript references, phases and deterministic dossier counters.
No task text, artifact bytes or private oracle answers are published.

| Epoch | Newly consumed turn | Input / output tokens | New settled episode | Transcript references |
| --- | --- | --- | --- | --- |
| 1 | 1 | 10 / 2 | `episode:0` | 0, 1 |
| 2 | 2 | 10 / 2 | `episode:2` | 2, 3 |

Both receipts have no interrupted or unmetered turns. Separate regressions retain
interrupted intents with and without measured usage, reject conflicting callback
retries and altered epoch assignment, and demonstrate that failed persistence
neither consumes an epoch nor permits unrecorded checker completion. Provider
usage also survives response-cap validation failure without receiving checker
credit. The existing archive tests retain manual grades, failed proposals,
prepared states and paired evidence across reopen, and preserve history during
rejected queries and parent-pointer revert.

Implementation: [evidence contract](../specs/epoch-evidence.md),
[test-first plan](../plans/epoch-evidence.md),
[journal design](../../crates/tinyrsi/src/evidence_journal/README.md).
Regression suites: `evidence_journal/test.rs`, `suites/test_journal.rs`,
`verification/test.rs` and the existing `archive/test*.rs` files.

Host admission must be stopped and runners joined at an epoch boundary. Upstream
transcript and episode reads are separate snapshots. A remote response lost
before its durable callback cannot be recovered; its invocation intent remains
explicitly unmetered for host reconciliation. This demonstration makes no
scientific improvement or ignition claim and does not close P2–P6.
