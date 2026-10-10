# Orchestrator

`run_proposal` processes one class-scoped proposal through grounded prediction,
exact deduplication, language guard, isolated memory pinning, screening, full
selection, independent confirmation and an explicit human gate.

`types.rs` contains proposal and host-port types. `mod.rs` contains the funnel
and class-to-activation mapping. `test.rs` pins rejection and promotion behavior.

The archive head is a selected design pointer, not proof that a live process
has applied it. The host executes the returned boundary and restores the
parent package/memory on revert. Evaluators enforce hard caps during execution
and never share mutable memory across arms. Every proposal needs a checkable
prediction and real public evidence. Raw ±2SE comparison is a heuristic;
scientific runs need normalized bootstrap scoring and frozen anchor suites.
