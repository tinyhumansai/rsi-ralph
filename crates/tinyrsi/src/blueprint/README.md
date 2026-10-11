# Verified decomposition

A `Blueprint` breaks one piece of work into statements that each need a host
check before they count. It is a directed acyclic graph of `BlueprintNode`s:
a node may only be verified after every node it depends on is verified. The
graph records state; it never runs a check itself.

| File | Responsibility |
| --- | --- |
| `mod.rs` | `Blueprint`: validation, `ready`, `report`, `is_complete` |
| `types.rs` | `BlueprintNode`, `NodeStatus`, `Verifier`, `VerifierKind` |
| `test.rs` | Cycle and identity rejection, invalidation of dependents |

## Design

`Blueprint::new` validates a nonempty DAG, unique identities, configuration,
dependency references and cycles. Initial verified states are rejected:
`report(id, passed)` is the host's explicit verifier outcome. A pass requires
verified prerequisites. A subsequent failed prerequisite invalidates verified
transitive dependents so they must be checked again.

`nodes` exposes immutable configuration and states; `ready` returns unverified
nodes with verified dependencies. `is_complete` requires every node verified.
The verifier ladder runs from proof through execution, schema, held-out reference,
rubric and preference. The last two are marked weak evidence.

## Public surface

| Item | Meaning |
| --- | --- |
| `BlueprintNode` | `id`, `statement`, `owner_role` (seat role), `dependencies`, `verifier`, initial `status` |
| `NodeStatus` | `Open`, `Sorry` (known incomplete), `Verified`, `Failed` |
| `Verifier` | `kind` plus a host-defined `spec` identifier |
| `VerifierKind` | `Proof`, `Execution`, `Schema`, `HeldOutReference`, `Rubric`, `Preference`; `is_weak()` is true for the last two |
| `Blueprint::new(nodes)` | Validates and returns the graph, or `Error::InvalidBlueprint` |
| `Blueprint::ready()` | Nodes not yet verified whose dependencies are all verified |
| `Blueprint::report(id, passed)` | Applies one host outcome; `Error::InvalidBlueprint` for an unknown node or a pass with unverified prerequisites |
| `Blueprint::is_complete()` | True when every node is verified |

A failed report sets that node to `Failed` and moves every verified node
whose dependencies are no longer all verified back to `Open`, repeatedly,
until nothing changes.

## Where it is used

`VerifiedRunner` (`src/verification/runner.rs`) takes a template `Blueprint`
whose nodes must match its `VerifierRegistry` declarations, and keeps one copy
per coordinator episode. It reports each host check into that copy and submits
coordinator completion only when the copy is complete. The per-node outcomes
also reach the observation dossier as `VerifierOutcome`s, which is how failed
nodes become the targets a proposal must cite.

## Operational constraints

- The host owns verifier execution. This graph does not call the
  coordinator's completion action or replace its episode lifecycle.
- A `Blueprint` is in memory. The types derive serde, but nothing in the
  crate persists a graph; restart reconstruction is the host's job.
- Node configuration is fixed at construction. `report` changes only
  `status`; there is no API to add nodes, edit dependencies or swap a
  verifier.
- `Rubric` and `Preference` nodes can be verified like any other, but they
  are weak evidence. Callers that need strong evidence must check
  `VerifierKind::is_weak` themselves.
