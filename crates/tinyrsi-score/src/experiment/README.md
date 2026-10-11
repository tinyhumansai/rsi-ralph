# Bounded orchestrator experiment

`ExperimentProtocol::new` preregisters original/evolved orchestrator artifact
identities, the identical initial solver hive, private selection/confirmation/
anchor suite fingerprints, normalization anchors and judge penalties, seed draws,
equal token caps, stopping count, evidence provenance, and efficiency margin.
Its SHA-256 digest exists before any model output. The immutable protocol cannot
be edited during execution. Reconstructing a changed specification produces a
new experiment, not an extension of the original one.

`run_experiment` invokes the host's `ExperimentExecutor::execute_trial` for both
arms under Full, MemoryOff, and RetreatOff interventions. There must be at least
three independent generation draws, at least two paired anchor seeds per draw,
and at least twenty total proposal attempts. Four draws and one attempt produce
24 assignments. Arm execution order alternates across draws. Every assignment
starts from a fresh copy of the same solver hive and memory state; only the
assigned immutable orchestrator configuration and explicit ablation may differ.

The host executes actual proposal generation, normalized screening, selection,
fresh confirmation, and the applicable approval gate. It returns the disposition
and exact generation/evaluation tokens for **every** attempt, including failed
calls and retries. Initial and final private anchor evaluations are additional
spend: the ledger adds their tokens to attempt evaluation spend. Equal caps apply
to the entire assignment, not only the accepted attempt. The host model port must
reserve input/output tokens before dispatch; post-hoc ledger checks cannot prevent
an untrusted callback from sending an oversized request.

Each returned assignment must exactly match its preregistration. Initial and
final anchor outcomes use the same complete task/family/seed matrix. Anchor seeds
are disjoint across independent draws; task/family identities remain fixed.
Original and evolved arms must have exactly identical initial outcomes under
matching seeds. Both per-run and aggregate caps are validated. Final normalized
H and all empirical family, false-claim, tamper, and cost gates are recomputed
from actual outcomes using the frozen scoring specification. No caller-provided
comparison or claimed gain is accepted as evidence.

The primary endpoint is evolved minus original **anchor H gain per 1,000 total
generation and evaluation tokens**. Its two-sided 95% percentile bootstrap
resamples complete independent paired trials; its draws are deterministic and
use the preregistered scoring seed and resample count. It does not treat proposal
attempts or individual task evaluations as independent trial replicates. Small
trial counts can still give unstable bootstrap intervals: report the pair count
and raw bounded protocol, and replicate a promising result independently before
making a broad efficacy claim. This is conditional evidence for the frozen task
population, model, and seed protocol, not a guarantee about unseen tasks.

Positive live evidence requires complete valid results from all assignments,
at least three full-mechanism pairs, the full-mechanism efficiency lower bound
above the declared margin, and accepted held-out anchor comparisons in every
evolved full-mechanism trial, with an accepted proposal in every trial, both
against its initial hive and against the final
original-arm hive. The report includes all three ablation endpoints. It does not
claim that an ablation's observed effect identifies a general causal mechanism.
A complete interval excluding the required gain is Negative. Missing evidence,
failed gates, or overlapping intervals are Inconclusive. Scripted callbacks
always produce ProtocolOnly; generated fixture scores cannot establish model
improvement. No verdict is called ignition, and one accepted proposal cannot
satisfy this protocol.

Host callback errors and invalid ledgers retain every requested attempt as
failed and conservatively charge both complete reservations. Such token totals
are reservations, not verified measurements; `verified: false` and a typed
failure remain in the report and prevent a positive live verdict. A callback
that exceeds a cap has invalid evidence; the report does not certify its actual
spend. Host error text is deliberately omitted because it may include private
inputs. The public serializable report contains scalar scores, uncertainty,
trial seeds, counts, spend/reservations, and the protocol digest. It contains no
private task identifiers, prompts, outputs, oracle answers, or per-task rows.
Raw `TrialEvidence` is deliberately not serializable. Durable execution/recovery,
private suite storage, hard model dispatch caps, learned orchestrator artifacts,
and final reporting belong to the host rather than this pure experiment ledger.
