# Private Lean checker validation

From the checkout root, generate a new private slice and verify its host
reference proofs with the actual installed pinned toolchain:

```sh
python3 scripts/lean/curate.py --output target/private-lean/slice.json
cargo run -p tinyrsi --features host --example verify_lean_slice -- \
  /absolute/toolchain/lean-4.34.1/bin/lake target/private-lean/slice.json
```

The input contains 12 agent-authored finite mathematical goals across screen,
selection, confirmation and anchor stages. Exact task IDs, disjoint execution
seeds and checker definitions freeze through `PrivateSuites`. The reference
proof `by decide` is checked by genuine Lake builds and kernel axiom reports.

Public output contains only toolchain identity, suite fingerprints, predefined
task IDs and observed build dispositions. The input, theorem statements, replay
seed, proof sources and diagnostic output remain under ignored `target/`.
These checks validate the adapter and numerical certificates; they do not
measure model performance or difficult general theorem proving. Never reuse
this slice as a human-curated independent benchmark.
