# Glossary

Terms used across the code, the module READMEs and `docs/rsi/`. Each entry
names the type or function that defines it; paths are relative to
`crates/tinyrsi/src/` unless they start with `crates/`, except `host/`,
`suites/`, `epoch_driver/` and `evidence_journal/`, which are relative to
`crates/tinyrsi-host/src/`. Read
[architecture.md](architecture.md) first for how the pieces fit together.

## The loop

**Epoch.** One pass from observed work to a possible deployment. Its number
comes from `DeploymentCheckpoint::epoch` (`host/types.rs`), which
`DeploymentStore::observe` increments each time it checkpoints an
observation. The frozen per-epoch settings are `Epoch` (`orchestrator/types.rs`:
id, allowed patch class, transcript refs, failure nodes, run budget) wrapped
by `HEpoch` (`orchestrator/normalized/types.rs`), which adds the scoring
policy and the three suite fingerprints. Not to be confused with
`EvidenceEpoch`, the evidence journal's receipt for the same window.

**Hive / package.** The thing being improved: a tinyhivemind
`tinyhivemind_lang::Package` (seats, prompts, context documents, memory
identities, roster, policies, goals). Re-exported as `tinyrsi::Package`.

**Proposal.** A model's suggested change, as `Proposal`
(`crates/tinyrsi-types/src/proposal/types.rs`): a title, one typed `Patch`, a mechanism, a
`Prediction` (`metric`, `minimum_gain`, the failure `nodes` it addresses),
public `transcript_refs`, risk and effort. Proposals are data; they never
apply themselves.

**Candidate.** The design that results from applying a proposal's patch to the
incumbent: `CandidateState` (`crates/tinyrsi-types/src/candidate/types.rs`) holds the new package
and its pinned memory watermarks. It is identified by its *content ID*.

**Content ID.** `sha256:` followed by 64 lowercase hex digits, computed by
`tinyhivemind_lang::lineage::content_id` over a package and its memory
watermarks. Archive records, deployment checkpoints and approvals all refer
to designs by this ID.

**Incumbent.** The currently selected design: the record returned by
`Archive::head` (`archive/mod.rs`). Every candidate is compared against it.
The incumbent is *selected*, which is not the same as *deployed* (see
**Deployment**).

**Archive.** The durable selection history, `Archive` (`archive/`). It keeps
founding and graded records, prepared candidates, declarations, paired
evaluations and approvals as ordered events in one JSON file.

**Lineage.** Two meanings. In the loop, the parent chain of accepted archive
records (`Record::parent`). Separately, `Lineage` (`lineage/mod.rs`), a small
in-memory step-ordered ledger of `GradeEntry` values. The two are not
connected.

**Deployment.** What a coordinator actually runs.
`DeploymentCheckpoint::deployed_id` (`host/types.rs`) records it;
`DeploymentStore::activate` changes it to the archive head, `rollback` to the
head's accepted parent. A pending `DeploymentIntent` blocks further changes
until `recover` runs.

## Generating proposals

**Generation.** Asking a model for proposals for one epoch:
`prepare_retreat_epoch` (`epoch_driver/generation.rs`), metered by
`GenerationMeter` and capped by `GenerationBudget`. It is *not* an
evolutionary generation of a population. The one exception in naming is
`GenerationBudget::generations`, which counts conversation turns per
variation inside the retreat Engine.

**Retreat.** The tinybackrooms creative "critic-pair" Engine used to
generate proposals (`GenerationMode::Retreat` in `epoch_driver/types.rs`;
request contract in `crates/tinyrsi-evolve/src/retreat/`). `RetreatWithoutMemory` disables recall;
`Direct` skips the Engine and makes one structured model call.

**Retreat memory.** `DurableRetreatMemory` (`crates/tinyrsi-evolve/src/retreat_memory/`): a bounded,
locked recall cache of discovery text and approved/rejected outcomes that the
retreat Engine reads. Not an audit log.

**Prior.** An optional host-supplied probability forecast for each typed
proposal occurrence, registered with `EpochJournal::register_proposal_priors`
after generation and before evaluation, then frozen (`priors` in
`epoch_driver/types.rs`). Outcome datasets score it against selection labels
only; see `epoch_driver/dataset/README.md`.

## Evidence

**Observation.** `Observation` (`observation/types.rs`): transcript sequence
numbers, seats and episode phases read by `Observer::snapshot`, with message
bodies removed.

**Dossier.** `Dossier` (`observation/types.rs`), built by `Dossier::build`
(`observation/mod.rs`): the public evidence for one epoch. It holds
transcript sequences, failed and open episodes and nodes, per-seat token
usage, wasted turns and memory-health counters. It is what a proposal model
may see; it contains no task text or oracle answers.

**Receipt.** A redacted, durable record that some work happened, safe to
publish. Three kinds exist: `EvidenceEpoch` (`evidence_journal/types.rs`),
which consumes measured turns into an epoch; `GenerationCallReceipt` inside a
`GenerationAudit` (`epoch_driver/receipt_types.rs`), one per formatter call;
and `RetreatEpochOutcome` (`epoch_driver/types.rs`), the per-proposal
classification of an epoch.

**Watermark.** A `u64` naming one immutable image of a memory namespace in
`SnapshotMemoryStore` (`memory/mod.rs`). A design's memory is the map from
identity to watermark.

**Canary.** A forbidden string (for example `TINYRSI_PRIVATE_CANARY_2026` in
the examples) that memory and retreat-memory stores refuse to accept, so
private content leaking into public text fails closed.

## Evaluating

**Stage.** `EvalStage` (`crates/tinyrsi-types/src/candidate/types.rs`): `Screen` (small replay and
regression check), `Selection` (the full selection suite) and `Confirmation`
(fresh private tasks and seeds that may not overlap earlier stages). Each has
its own frozen suite fingerprint in `HEpoch::suite_ids`.

**Suite / private task.** `PrivateTask` and `PrivateSuites` (`suites/`): a
private prompt, an immutable `HostVerifier`, a family and seeds. Suites are
fingerprinted so an epoch can prove which tasks it used without revealing
them.

**Trial (evaluation).** One paired run: incumbent and candidate on the same
task and seed, returned as `EvalPair` / `HEvalPair`. Each arm gets fresh
coordinator storage in `CoordinatorEvaluator`.

**Trial (experiment), arm, ablation.** In `crates/tinyrsi-score/src/experiment/`, a *trial* is one
`TrialRequest`: an *arm* (`ExperimentArm::Original` or `Evolved`
orchestrator configuration) crossed with an *ablation*
(`ExperimentAblation::Full`, `MemoryOff`, `RetreatOff`) and a seed draw.
`run_experiment` executes every assignment through the host's
`ExperimentExecutor`.

**H score.** The normalized hive score from `score` and `compare_h`
(`crates/tinyrsi-score/src/scoring/mod.rs`): each family's mean quality is normalized against its
anchor, combined by an interquartile mean, then penalized for false claims,
tampering and excess cost. `compare_h` adds a paired cluster-bootstrap
interval and returns `Accept`, `Reject` or `Inconclusive`. Full formula in
`crates/tinyrsi-score/src/scoring/README.md`.

**Anchor.** `FamilyAnchor` (`crates/tinyrsi-score/src/scoring/types.rs`): a frozen `baseline` and
`reference` quality for one task family, used to normalize H. Separately,
`AnchorLedger` monitors anchor-suite scores across epochs, and the
experiment's *anchor suite* is a held-out draw measured before and after
each trial.

**Reason code.** `ReasonCode` (`crates/tinyrsi-types/src/proposal/types.rs`): why a proposal
stopped or was accepted, from `Untestable` and `Ungrounded` through `NoGain`,
`NotReplicated` and `ApprovalDenied` to `Accepted`. `HostFailure` means the
evaluator or approval service failed, not the proposal.
`Archive::proposal_dispositions` (`archive/dispositions/`) lists these
reasons in order and splits `Duplicate` into `DUP_ACCEPTED`, `DUP_REJECTED`
and `DUP_UNRESOLVED` by what the earlier identical patch led to.

## Changing the hive

**Patch.** A `tinyhivemind_lang::patch::Patch`: a list of typed `Operation`s
and one `PatchClass`. Applied by `patch::apply` inside the orchestrator.

**Patch class.** `PatchClass`: `Context`, `Memory`, `Prompt`, `Roster`,
`Topology`, `Policy` or `Goal`. Each epoch allows exactly one class
(`Epoch::class`); a proposal of another class stops as `WrongClass`.

**ApplyAt.** `ApplyAt` (`orchestrator/types.rs`): the activation boundary an
approved change would need, derived from its class by `apply_at`
(`orchestrator/mod.rs`): `NextTurn` for context and memory, `ReplaceAgent`
for prompts, `ManageRoster` for roster, `BetweenSessions` for topology,
policy and goals. The host does not act on it yet; `CoordinatorHost` always
rebuilds.

**Blueprint.** `Blueprint` (`crates/tinyrsi-verify/src/blueprint/`): a dependency DAG of
`BlueprintNode`s, each with a `Verifier` kind. Nodes become verified only
through the host's `report`. The host's per-node checker results
(`VerifierOutcome` with a `NodeStatus`) become the dossier's `failed_nodes` /
`open_nodes`, which proposals must cite in `Prediction::nodes`.

**Verifier.** `VerifierKind` (`crates/tinyrsi-verify/src/blueprint/types.rs`) ranks evidence from
`Proof` down to `Rubric` and `Preference` (the last two are weak).
`HostVerifier` (`crates/tinyrsi-verify/src/verification/types.rs`) is the concrete check: `Process`,
`Golden`, `Schema`, `Lean` (a restricted proof block checked directly) or
`Lake` (a frozen project built with a pinned Lean toolchain; see
`crates/tinyrsi-verify/src/verification/lake/README.md`).

**Approval gate.** `HApprovalGate::approve_h`
(`orchestrator/normalized/types.rs`): the only path from a confirmed
candidate to `Accepted`. `ApprovalGate` is the same boundary for the raw
`run_proposal` funnel.

## Lineage ledger

**GradeEntry.** `crates/tinyrsi/src/lineage/types.rs`: step, snapshot
address, grade, verdict and rationale; one row of the `Lineage` ledger.
