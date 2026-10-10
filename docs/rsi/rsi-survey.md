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
near-tie robust overrides. About a quarter of graded rewrites scored higher
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
