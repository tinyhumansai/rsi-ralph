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

Failed host attempts are also deduplicated. A prepared snapshot is retained for
audit, not treated as a resumable evaluation checkpoint. Retrying private
evaluation requires a separately specified recovery protocol that pins the
incumbent, epoch, suites, seeds, budget and policy, and distinguishes unobserved
infrastructure failures from partially observed trials. Reusing package and
watermarks alone does not establish those invariants. This API therefore does
not automatically retry `HostFailure` attempts; host recovery is part of P2.
