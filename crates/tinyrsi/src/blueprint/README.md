# Verified decomposition

`Blueprint::new` validates a nonempty DAG, unique identities, configuration,
dependency references and cycles. Initial verified states are rejected:
`report(id, passed)` is the host's explicit verifier outcome. A pass requires
verified prerequisites. A subsequent failed prerequisite invalidates verified
transitive dependents so they must be checked again.

`nodes` exposes immutable configuration and states; `ready` returns unverified
nodes with verified dependencies. `is_complete` requires every node verified.
The verifier ladder runs from proof through execution, schema, held-out reference,
rubric and preference. The last two are marked weak evidence.

The host owns verifier execution. This graph does not call the coordinator's
completion action or replace its episode lifecycle.
