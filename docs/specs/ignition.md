# Bounded evidence for orchestrator improvement

An ignition experiment measures whether a frozen evolved orchestrator improves
the same starting solver hive more efficiently than a frozen original
orchestrator. A positive single proposal is not ignition. A small bounded run can
produce useful negative or inconclusive evidence without supporting ignition.

## Preregistration

Before any model output, freeze immutable orchestrator and initial hive artifact
identities, disjoint private selection/confirmation/anchor suite fingerprints,
family reference anchors, verifier/judge policy, generation and evaluation seed
draws, token caps, stopping count, provenance, and practical efficiency margin.
Hash the full validated protocol. Changes require a new experiment digest.
The host binds these identities to real artifact and private suite bytes.

Use both original/evolved arms under memory+retreat enabled, memory disabled,
and retreat disabled. Start every assignment from the identical initial solver
hive and reset memory and archive state. Hold the solver model, verifier policy,
and all caps fixed. Use at least three independent generation trials with at
least two paired anchor seeds each, and at least twenty proposal attempts in
total. Four trials, three ablations, two arms, and one attempt give 24 attempts.
Counterbalance execution order across trial draws. No adaptive stopping after
seeing a promising score is allowed.

## Execution and measurement

Run actual model callbacks and the normalized proposal runner. Include rejected,
unresolved, and failed proposal attempts in the ledger. Count all generation,
retry, screening, selection, confirmation, and initial/final anchor evaluation
tokens against each assignment's equal total caps. The host reserves hard input
and output caps before dispatch; the experiment rejects malformed or oversized
returned evidence rather than retrospectively certifying it.

Keep anchor tasks out of proposal and reusable solver contexts. Task prompts
enter only their isolated evaluation runs; oracle answers remain outside both
solver and proposal channels. Match original and evolved draws exactly, including identical
starting anchor outcomes. Recompute initial/final normalized H and independent
hard family/health/cost gates from complete paired anchor matrices. Do not use a
self-reported objective, generated text, or synthetic script output as efficacy
evidence. When a proof checker is unavailable, report unavailable verification;
never emulate Lean or label an unchecked proof as machine verified.

The endpoint is evolved minus original anchor H gain per 1,000 total tokens.
Report a deterministic two-sided 95% percentile-bootstrap interval resampling
independent paired trials. Include all ablation estimates, pair counts, failure
counts, and token spend. Avoid pseudoreplication from task rows or proposal
attempts within a trial. Small-sample bootstrap intervals and host-owned health
rate denominators must be stated as limitations.

## Evidence decision

SupportedImprovement requires complete valid assignments, at least three full
pairs, an efficiency lower bound above the preregistered margin, and accepted
anchor H comparisons for every evolved full trial against both its initial hive
and the final original hive, with an accepted proposal in every evolved full
trial. Negative means the complete interval excludes the
required gain. Missing evidence, failed gates, or overlapping intervals are
Inconclusive. Scripted evidence is always ProtocolOnly. These conclusions apply
to the frozen protocol and never assert ignition or open-ended self-improvement.

Errors retain scheduled attempts as failed and charge complete token
reservations. These are explicitly unverified usage, prevent positive claims,
and do not certify an invalid callback's actual spend. The public report exposes
only aggregate scores, bounds, seeds, counts, spend/reservations, failure types,
and the protocol fingerprint. It must omit private task IDs, prompts, model
outputs, and oracle text. Independent replication and larger held-out trials are
necessary before expanding an efficacy claim beyond the bounded protocol.
