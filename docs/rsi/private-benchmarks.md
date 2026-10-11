# Private numerical inputs and hard public controls

The user requested agent-authored benchmark inputs and hard problems with
published solutions. This curation adds 48 fresh numerical tasks, independently
checked before any model trial, plus a verified catalog of nine public Euler
controls. It does not report model performance or recursive improvement.

## Published hard problems

Official difficulty percentages were checked on 2026-10-11. Each linked solution
exists at immutable Project Nayuki revision
`12f3dbc66ab9b7905c6cfe6f2fb4810583307923`. The
[source catalog](results/2026-10-11/euler-source-catalog.json) records fetched
source hashes, exact published answers and the answer-list hash. Published
reference programs were not executed as part of this source-availability audit.

| Problem | Official difficulty | Reason to include | Published solution |
| --- | --- | --- | --- |
| [433: Steps in Euclid's Algorithm](https://projecteuler.net/problem=433) | 92% | Summing division counts over enormous ordered-pair ranges demands a structural counting argument. | [Java](https://github.com/nayuki/Project-Euler-solutions/blob/12f3dbc66ab9b7905c6cfe6f2fb4810583307923/java/p433.java) |
| [222: Sphere Packing](https://projecteuler.net/problem=222) | 49% | Geometry combined with combinatorial ordering optimization. | [Python](https://github.com/nayuki/Project-Euler-solutions/blob/12f3dbc66ab9b7905c6cfe6f2fb4810583307923/python/p222.py) |
| [280: Ant and Seeds](https://projecteuler.net/problem=280) | 41% | Large constrained Markov state space and numerical stopping criteria. | [Python](https://github.com/nayuki/Project-Euler-solutions/blob/12f3dbc66ab9b7905c6cfe6f2fb4810583307923/python/p280.py) |
| [500: Problem 500!!!](https://projecteuler.net/problem=500) | 40% | Prime-power allocation for an enormous divisor count with a modular result. | [Python](https://github.com/nayuki/Project-Euler-solutions/blob/12f3dbc66ab9b7905c6cfe6f2fb4810583307923/python/p500.py) |

The catalog also includes #186, #345, #387, #407 and #425 as moderate controls
and sources of algorithmic families. Public Euler questions and solutions may
appear in model training. Keep their results separate from the private draws;
they are useful controls but cannot establish contamination-free generalization.

## Fresh private draws

Each of screen, selection, confirmation and anchor contains three tasks from
each of four families:

| Family | Independent answer methods | Instance scale |
| --- | --- | --- |
| Assignment | Subset dynamic programming; primal-dual augmenting paths | Fresh signed matrices, dimensions 11–15 |
| Harshad primes | Right-truncatable prefix tree plus trial primality; dense sieve plus complete prefix checks | Fresh strict bounds 50,000–199,999 |
| Idempotents | Prime-power CRT enumeration; direct modular residue search | Fresh inclusive bounds 700–1,999 |
| Euclidean steps | Direct ordered-pair division counts; reverse Euclidean forest traversal | Fresh inclusive bounds 250–699 |

These reduced bounds make dual independent checks practical. Their difficulty
is not the official rating of the original Euler problem. Task prompts specify
all mathematical conventions and require one canonical integer with newline.
Exact-byte golden checkers verify the answer; they do not certify the algorithm
or a Lean proof.

The generator draws a private 256-bit OS seed, records it privately for replay,
requires unique prompts and execution seeds, and rejects oracle disagreements.
The Rust adapter rejects malformed, overlapping and unbalanced stages and freezes
actual `PrivateTask` and `PrivateSuites` values. Both trial arms must load this
same bank; confirmation and anchor use their own frozen draw. Freeze inputs and
retain their hashes before any evaluated calls.

[Curation evidence](results/2026-10-11/private-bank-curation.json) contains the
counts, bank hash and four actual suite fingerprints. The bank also contains
96 private calibration probes: one correct answer and one off-by-one answer per
task. The Rust adapter observed all 96 labels through the actual host golden
checker. These are automatic ground-truth controls for future judge studies;
they do not measure the calibration of any judge or model. The bank itself is in
the worktree's ignored `target/private-benchmarks/bank-with-labels.json`, accessible only to
the host. Do not pass it to a proposal model or publish it. Individual evaluation
solvers receive their assigned prompt; answer oracles remain host-only.

## Reproduction and remaining criteria

Follow [curation commands](../../scripts/benchmarks/README.md). The bank generator
refuses existing output. For a new independent experiment, use a fresh filename;
for a repeated arm, reuse the frozen original bank.

These are agent-authored, automatically cross-checked tasks. There are no
human-rated calibration labels and no measured family normalization anchors.
Anchor tasks now exist, but anchor values must come from actual baseline runs
under frozen budgets. Code, private-reference and harness-engineering families,
the pinned `lake build` slice, empirical calibration and informative ablations
remain outstanding. Keep issues #4, #5 and #6 open until their other criteria
are verified.
