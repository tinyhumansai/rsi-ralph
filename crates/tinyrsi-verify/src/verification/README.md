# Host verification and controlled completion

`VerifierRegistry` freezes node identities, authorized seats and strong host
checks. `verify` requires exact equality with an independently constructed
`VerificationBinding`; a model-supplied binding alone is not authorization.
Evidence contains identity, transcript sequence references, artifact SHA-256,
checker configuration and executable SHA-256, finite zero/one verified credit
and subprocess exit status. Historical journal evidence written before checker
identity was added retains a missing identity; new callbacks require it. Golden values,
schemas, artifact bodies and command output remain private to the host.

`Process` runs a direct executable with frozen arguments. `{artifact}` and `{workspace}`
argument placeholders name the staged file and directory; expansion occurs
inside direct argv strings, never through a shell. Sources and compiler outputs live beneath the
host's `CommandSpec.workspace_root`, which should be under checkout `target/`.
The child receives a cleared environment and a basic system PATH. Time and
combined stdout/stderr limits cause failure, with output never entering public
evidence. This is a checker port, not an operating-system sandbox: host commands
executing generated programs must supply a sandbox wrapper appropriate to the
threat model. The host owns executable trust and toolchain setup.

`Lean` accepts only a generated `by` proof block appended to a frozen theorem
named `rsi_goal`. It rejects admissions, new declarations, options, elaborator
execution and comments before invoking the host checker. Success additionally
requires a kernel axiom report whose only dependencies are the stock
`propext`, `Classical.choice` and `Quot.sound` axioms. The host must configure the
actual stock Lean executable, not a model-supplied or simulated checker.
Exactly one report for the frozen theorem is required; duplicate or conflicting
reports reject a generated trace that tries to impersonate kernel output.
Checker temporary files also live beneath the staged workspace.

`VerifiedRunner` owns a separate Blueprint per episode and selects only ready
nodes owned by the active seat. The producer receives no Coordinator handle.
Forged bindings, failed verifiers and incomplete DAGs park the turn. Completion
is submitted inside the active runner future only after the required DAG is
verified. Raw Coordinator callers are outside this boundary and can still
submit their own completion actions. For multi-seat DAGs, the host must release
parked seats when their dependencies become ready; seat membership retirement
must not be counted as verified success.

`VerifiedRunner::with_observer` installs a host evidence callback after a real
checker and before its DAG/completion update. A failed callback parks the turn
without submitting completion. `ControlledRunnerFactory::with_evidence_journal`
uses this port together with provider usage callbacks to persist measured
evidence before allowing its host outcome to finalize.

`Lake` uses the same restricted proof boundary inside a fresh pinned project.
Both actual compiler binaries and the immutable project template are included
in private suite identities. See [checker constraints](lake/README.md) and the
[private slice example](../../../tinyrsi-host/examples/verify_lean_slice/README.md).
