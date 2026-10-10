# Verified decomposition

A host owns a DAG of blueprint nodes, each with a statement, role, dependencies,
a verifier definition and an explicit open/sorry/failed/verified state.
Dependencies must exist, identities must be unique and cycles are invalid.
Only a host-observed verifier outcome can mark a node verified. Ready nodes
have verified dependencies. Completion requires every node verified; self-report
is insufficient. Proof, execution, schema, held-out reference, rubric and
preference form a strength ladder; the last two are weak evidence.

The blueprint is runtime state beside the immutable verifier configuration;
patches cannot rewrite the judge. Scores use eval-owned checkpoints, never the
hive's self-selected decomposition.
