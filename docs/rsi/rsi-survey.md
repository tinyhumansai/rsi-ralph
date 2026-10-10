# RSI Landscape Survey

Notes from the literature, gathered 2026-10-10. Primary source read in full;
others summarized from abstracts and secondary coverage.

## 1. Primary: "Recursive self-improvement of AI research agents" (Weco AI,
arXiv 2609.26457v1, Sep 2026)

### What the system does

- **Bi-level optimization.** Inner loop: a code-optimization agent (tree
  search over candidate solutions, operators = draft / debug / improve,
  greedy parent selection) improves code against a measurable metric under a
  fixed dollar budget. Outer loop: a stronger agent reads the incumbent agent
  code plus its grade history and proposes a **rewrite of the agent itself**.
  Each accepted rewrite becomes the incumbent the next step edits.
- **Grading.** Each candidate agent runs on a diverse selection benchmark
  (three task families: ML engineering, heuristic algorithm engineering,
  *harness engineering*), multiple seeds per task, solutions scored on
  **private held-out data**, scores aggregated into a private grade. Rewrite
  accepted only if the private grade improves. Fixed evaluation budget per
  agent so gains reflect better algorithms, not more compute.
- **Signal separation.** Inner-loop optimization signal (public, agent
  visible) is deliberately decoupled from outer-loop selection signal
  (private) to prevent the agent directly optimizing the selection criterion.

### Key results (8-day autonomous run, 100-node trajectory)

- Seven accepted improvements; incumbent grade 0.703 → 0.778. Two more runs
  accepted 2 and 4 rewrites. Sustained trend, not a one-off.
- Gains transfer to four held-out benchmarks (ALE-Bench, MLE-Bench,
  FML-Bench, WeatherBench 2 — last is out-of-distribution). Evolved agents
  match or beat a 2-year human-engineered production agent.
- **Emergent reduction in reward hacking**: 55% → 32% on a held-out kernel
  task family, below the human baseline's 39%, never explicitly optimized.
- **Ignition test** (does an evolved agent drive the loop better than its
  creator?) was inconclusive: no degradation, possible sample-efficiency
  gain (treatment arm plateaued ~step 20 vs ~40), but 3 seeds too few.

### What the loop actually discovered (all at the harness layer)

1. **Bandit selection over drafting strategies with periodic forking**
   (UCB1 over five strategy arms, 30% softmax sampling, fork the global best
   every 5 steps) — replaces greedy parent selection. Solves plateauing.
2. **Bounded, role-specific prompts**: compact summaries instead of full
   history; per-call prompt size held roughly constant (vs unbounded growth).
3. **Bug-rate-gated failure memory**: when candidate bug rate ≥ 15%, inject
   up to 3 deduplicated recurring error signatures into draft/improve prompts.
4. **Prompt-level robustness safeguards**: fixed instruction that solutions
   are scored on a private split (prefer generalizable approaches); re-prompt
   when generated code is a near-empty stub (< 40 chars).
5. **Median-distance selection penalty** against lucky one-off high scores
   (order-preserving, so never changed the actual choice in replay).
6. **Fixed a broken held-out evaluation script** rather than exploiting it —
   the agent repaired its own grading harness when it found a crash bug.

### Rejected proposals (Appendix D) — what evolution said no to

Island populations with migration, LLM-judge tournaments, restart policies,
majority-vote ensembles (budget better spent on more search steps),
explore-rate decay, variance-adaptive exploration, MCTS-style value backup,
near-tie robust overrides. About a quarter of rejected, graded rewrites scored higher
than the incumbent on the agent-visible public signal and were still rejected
on the private grade — the decoupling did real work.

### Failure modes and limits worth designing around

- **Noise compounds across both loops.** One falsely accepted rewrite from a
  noisy grade derails the outer loop's subsequent search. Statistical gating
  of acceptance is a live research question.
- Discovered agents become **complex and hard to interpret**; unclear which
  components drive performance vs are dead artifacts.
- Cost of evaluation dominates; ignition-style claims are compute-limited.

## 2. Related systems

| System | Idea | Lesson for us |
|---|---|---|
| Darwin Gödel Machine (arXiv 2505.22954) | Archive of self-rewriting coding agents; open-ended evolution, keeps ancestors selected by descendant success | Maintain an archive/lineage of agent configs, not a single chain; let branching selection pick parents |
| Gödel Agent (arXiv 2410.04444) | Agent modifies its own runtime logic at runtime, unrestricted design space | Full self-reference is possible but unconstrained search is expensive and risky; scope the mutable surface |
| Mendel Gödel Machine (arXiv 2608.07645) | Comparative evolution: derive mutations from the archive of past attempts, not just one failure trajectory | The archive itself is a learning signal — OH memory is a natural home for it |
| GEPA (arXiv 2507.19457) | Reflective *prompt* evolution beats RL with far fewer rollouts; natural language as a learning medium | Prompt deltas are cheap, interpretable, and effective — the highest-value first mutable surface |
| TextGrad / DSPy lineage | Text components as differentiable-ish parameters, natural-language feedback as gradients | Structured feedback artifacts between grading and rewriting improve proposal quality |
| AlphaEvolve / OpenEvolve / FunSearch family | Population-based evolution of programs with quality-diversity archives | Island models and archives; note the Weco loop *rejected* island populations for its scale — start simple |

## 3. Design implications extracted

1. **Optimize the harness layer, not weights.** Proven effective; matches OH's
   configurable surface (prompts, skills, workflows, memory rules, tool
   routing, model routing).
2. **Private held-out grading is non-negotiable.** Public signal selects
   candidates; a decoupled private signal accepts them.
3. **Diverse selection benchmark → general mechanisms.** Task diversity
   pressures the loop away from task-specific tricks.
4. **Fixed budget per evaluation** so improvements = efficiency, not spend.
5. **Archive + lineage beats single chain** for robustness (DGM), though the
   Weco loop itself ran a chain; archives cost more bookkeeping.
6. **Watch reward hacking explicitly** even if the loop optimizes something
   else; a behavior metric never optimized for is a good canary.
7. **Noise control is the weak point** of every system in this line; build
   multi-seed grading and acceptance margins in from day one.

## Citation audit (2026-10-10)

The arXiv abstract pages below were fetched and their citation-title metadata
matched to the issue references. This verifies identifiers and titles, not every
empirical conclusion. The Weco v1 HTML was also checked for the 100-node run,
seven accepted improvements, 0.703→0.778 grades, 55%→32% reward-hacking rates,
ignition discussion and rejection statistics. The quarter figure refers to
**rejected graded rewrites**, not all proposals. These are reported results,
not experiments independently replicated here.

| Work | Verified arXiv identifier | Design use |
| --- | --- | --- |
| SICA | [2504.15228](https://arxiv.org/abs/2504.15228) | Improve the coding harness |
| ADAS | [2408.08435](https://arxiv.org/abs/2408.08435) | Archive agent designs |
| STOP | [2310.02304](https://arxiv.org/abs/2310.02304) | Bound self-referential editing |
| MASS | [2502.02533](https://arxiv.org/abs/2502.02533) | Stage prompt and topology edits |
| GPTSwarm | [2402.16823](https://arxiv.org/abs/2402.16823) | Typed graph optimization |
| MaAS | [2502.04180](https://arxiv.org/abs/2502.04180) | Architecture families |
| DyLAN | [2310.02170](https://arxiv.org/abs/2310.02170) | Diagnose seat contribution |
| ACE | [2510.04618](https://arxiv.org/abs/2510.04618) | Itemized context deltas |
| Dynamic Cheatsheet | [2504.07952](https://arxiv.org/abs/2504.07952) | Consolidated memory |
| Draft-Sketch-Prove | [2210.12283](https://arxiv.org/abs/2210.12283) | Verifiable subgoals |
| LeanDojo | [2306.15626](https://arxiv.org/abs/2306.15626) | Formal verifier execution |
| Voyager | [2305.16291](https://arxiv.org/abs/2305.16291) | Verify before retaining a skill |
| OMNI | [2306.01711](https://arxiv.org/abs/2306.01711) | Novelty as a search signal |
| Promptbreeder | [2309.16797](https://arxiv.org/abs/2309.16797) | Prompt proposal diversity |
| Sycophancy to Subterfuge | [2406.10162](https://arxiv.org/abs/2406.10162) | Track objective tampering |

The Weco, DGM, GEPA and Gödel Agent identifiers in the earlier sections were
also verified against title metadata. References without concrete links in
the umbrella issue (including Lean blueprints, AlphaProof, CodeT, AgentVerse
and the AI Scientist line) are conceptual pointers here; their individual
claims need source-specific review before being used as empirical evidence.

The implementation follows the three common structures: typed genotype,
immutable external judge, durable archive. Its local arithmetic smoke run
does not reproduce any cited research benchmark or establish ignition.
