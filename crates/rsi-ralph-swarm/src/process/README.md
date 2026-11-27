# Process harness seats

The seats `SwarmRequest::seat_harnesses` names run on a process harness,
started by tinyhivemind's `ProcessSeats` under the swarm's coordinator; see
the crate README's "Process harness seats" for what such a seat gets.

| File | Purpose |
| --- | --- |
| `mod.rs` | Which seats run on a process, the refusals before the run, the lowered seat and host tools each gets, start and stop |
| `tools.rs` | The host tools (`read_file`, `list_files`, `search_files`, `write_file`, `edit_file`, `shell`) and the package tool names each stands in for |
| `installs.rs` | `harness_installs`: the harness programs named by `TINYRSI_*` variables, and the versions they must report |
| `test.rs` | Process seats, their refusals, the lowered seat per harness, host tools by allow list and access, and the installs |

Constraints:

- **The swarm's settings stay with the swarm.** The lowered seat a harness
  receives carries only the prompt, the model and the harness. The
  package's `config`, tool lists and model settings are the swarm's to
  honour, and tinyhivemind refuses them on a process seat.
- **Host tools stay in the seat's checkout.** Their paths are relative to
  it; an absolute path, or one that climbs out with `..`, is refused.
- **Process seats are always stopped.** This happens after the drain,
  whether or not the run succeeded, before the coordinator is dropped.
