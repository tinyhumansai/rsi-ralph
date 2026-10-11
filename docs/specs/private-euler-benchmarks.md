# Host-owned Euler-inspired benchmark inputs

Related: issues #1, #4 and #6. The user authorized agent-authored inputs and
requested hard problems with published solutions.

Maintain a public source catalog including official problem URLs, checked
difficulty, immutable published solution URLs and source hashes. Original Euler
questions are public controls, never described as private or contamination-free.
Answer provenance distinguishes a published answer from an independently run
checker. Do not redistribute external solution source in the repository.

Generate fresh private parameterized tasks using an operating-system random
seed. Freeze four disjoint stages and four mathematical families. Compute every
answer using two structurally distinct algorithms and reject disagreements.
Store prompts, seed draws and exact-match answer oracles only under gitignored
`target/`, with restrictive permissions. Refuse to overwrite an existing bank.
Published evidence contains counts, source provenance and opaque fingerprints.

Provide an offline Rust adapter that reads the private bank, validates its format
and freezes actual `PrivateTask`/`PrivateSuites` values with immutable golden
checkers. It prints only fingerprints and the count of automatic calibration labels
observed through the actual host checker. Each task contributes one correct
and one off-by-one answer; no artifact contents or artifact hashes escape. No proposal model receives the bank or
the answer keys. Evaluation solvers receive their assigned prompt as usual.

These tasks are agent-authored numerical mathematics. They do not substitute
for a pinned Lean slice, code/harness suites, human calibration, measured family
anchor values or informative empirical improvement experiments.
