# Host-owned numerical benchmark curation

These standard-library-only Python tools create original private variants and
check public solution provenance. No downloaded solution is executed or copied
into this repository. Run from the repository root:

```sh
python3 -m unittest discover -s scripts/benchmarks -p 'test_*.py'
python3 scripts/benchmarks/catalog.py > target/euler-source-catalog.json
python3 scripts/benchmarks/curate.py --output target/private-benchmarks/bank.json
cargo run -p rsi-ralph-host --example private_benchmark -- \
  target/private-benchmarks/bank.json
```

`catalog.py` needs network access and checks official metadata against published
solution headers and the pinned answer list. `curate.py` uses operating-system
randomness, recomputes every answer with two independent algorithms and stores
the private seed for reproducibility. It refuses output outside `target/` and
refuses to overwrite an existing bank. New bank directories have mode 0700;
bank files have mode 0600. Never copy bank contents into a commit or model prompt.

`oracles.py` contains original paired implementations: subset DP versus
primal-dual assignment; prefix-tree trial division versus prime-sieve prefix
testing; CRT residue reconstruction versus direct residue search; forward
Euclidean division versus reverse Euclidean forest traversal. Their tests use
known examples, boundary cases and deterministic random matrices.

The catalog is a public-control source list, not evidence of model performance.
The private banks contain mathematical numerical tasks, not formal proofs or
code execution assessments. Smaller variants are intentionally tractable for
independent oracle construction; the original hard controls remain separate.
