# Agent-authored private Lean slice

`curate.py` draws a fresh OS seed and creates 12 finite mathematical goals:
Euclidean division certificates, modular idempotents and modular inverses.
Each of four stages has three distinct tasks and globally unique execution
seeds. Repeated numerical statements are resampled across stages, and the
Rust adapter rejects overlapping theorem inputs. Python constructs the statements; the actual Lean kernel independently
checks the reference proofs through the `verify_lean_slice` example.

```sh
python3 -m unittest discover -s scripts/lean -v
python3 scripts/lean/curate.py --output target/private-lean/slice.json
```

The input uses mode 0600, refuses overwrite and must stay below checkout
`target/`. Retain it privately for replay. The generator prints counts and the
exact toolchain identity only. These small automatically constructed
certificates are checker validation inputs, not human labels or a measurement
of difficult mathematical reasoning. Hard public Euler controls and published
solutions are catalogued separately in `scripts/benchmarks/`.
