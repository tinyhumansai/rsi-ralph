# Durable normalized retreat epochs

`prepare_retreat_epoch` consumes a genuine persisted observation, its redacted
Dossier, an immutable normalized-H epoch policy, a frozen public orchestrator
instruction string and a separately chosen generation seed. It validates that
transcript references and failed/open nodes agree before calling any model.
Orchestrator instructions enter the actual Engine brief and direct proposer
input; both the instructions and generation seed enter the journal fingerprint.

Every proposal schema requests `prediction.metric = normalized_h` before the
provider call. Existing raw-quality generation remains a separate API. Generated
predictions are never retrofitted or relabeled. Engine mode uses the real
`tinybackrooms::Engine`, the bounded `ModelConversant`, and the real durable
Memory adapter. Memory-off removes the request namespace. Direct mode calls the
same native structured provider once, under the same frozen budget and schema,
without a critic, ranking or recall. Arms share caps; actual spending can differ.

`GenerationBudget` controls breadth, items, selection, judge repetitions and
model reservations. Four variations producing five items with top-k twenty can
return twenty ranked proposals in one Engine run. The host must check actual
counts: missing items never trigger invented fallbacks. Accounting charges
failed calls and records invalid formatter attempts even when later repaired.
A memory warning stops preparation because silent loss of durable discovery
would invalidate the requested arm.

`PreparedRetreatEpoch.ranked_items` distinguishes selected ranking entries from
all strictly decoded formatter occurrences. Selected proposals come first;
unselected occurrences follow without deduplication. Every actual typed attempt
can reach the RSI funnel, where repeated patches receive `Duplicate` outcomes.
`EpochJournal::generation_receipts` returns versioned safe formatter-call rows.
Each call receives an ID before dispatch; each item keeps its original offset,
including duplicates and repairs. Fixed dispositions distinguish schema, strict
decoding and public-text guard failures. Backend failures and malformed envelopes
receive call-level statuses. Receipts describe generation, not host evaluation.

Sidecars named `<journal>.generation-<epoch>.json` contain no generated bodies,
raw errors, prompts or output digests. Atomic fsynced writes precede exposure to
the Engine; a write failure prevents successful preparation. Callback lifetimes
retain the journal's exclusive writer lock. Pending calls survive interruption
and never trigger an implicit retry. Copy these sidecars together with the private
journal when restoring a run. Legacy journals without receipts return `None`;
new journals reject missing, corrupt or mismatched sidecars.

`EpochJournal` exclusively locks a versioned host-private snapshot, records an
intent before model calls and atomically persists strictly decoded proposals.
Reopening a prepared epoch returns those proposals without another provider
call. Input changes reject reuse. Interrupted generation or evaluation with
unknown external effects fails closed; the host must resolve the interruption
or start a new genuinely observed epoch, not manufacture a replacement result.
Raw invalid model outputs and private oracle responses never enter the journal.

`evaluate_retreat_epoch` runs synchronously, separately from the generation
executor, so a real Coordinator evaluator may own its Tokio runtime. It feeds
ranked proposals to `run_proposal_h`, retains redacted classifications, and
stops after acceptance. Reopening a completed evaluation repeats neither private
draws nor the human decision. The explicit `HApprovalGate` remains mandatory;
accepted archive selection does not silently activate a runtime. The caller
uses `DeploymentStore` to activate or roll back exact accepted snapshots.

[Selection-only outcome datasets](dataset/README.md) connect these occurrence
identities to frozen priors, measured selection labels and descriptive metrics.

## Loop trace

`record_epoch` writes an evaluated hive epoch onto the loop trace
(`tinyrsi_trace::TraceSink`), so `apps/rsi-viz` shows hive epochs exactly as it
shows script runs. Call it after `evaluate_retreat_epoch`: hive candidate ids
exist only once a proposal is formalized, and the visualizer pairs ideas with
candidates through `ideated.candidate`. It emits one `ideated` per proposal
(`p<epoch>-<i>`, naming its candidate, or none when it was never formalized
or duplicated an evaluated patch), then for each candidate this
epoch evaluated `measured` (the H estimate), `scored` (accepted only after human
approval) and `improved` when accepted, and finally `epoch_finished` with the
accepted H or the incumbent's H on the epoch's draw. Evaluations are bound to
the epoch, incumbent, proposal and candidate; a never-formalized proposal or a
duplicate ran nothing and gets no `measured` or `scored`. A `HARMFUL`
candidate is measured from its `CandidateFailureEvent`: no mean, its failed
runs as `failures` and the runs attempted as `seeds`. The host still emits
`run_started`, `epoch_started`, `observed` and `run_finished`, and chooses the
epoch label, because only it knows where its run begins and what it observed.
`examples/complete` shows the full sequence.

## Penalty feedback

`prepare_retreat_epoch` turns every archived stop into a `PriorIdea` seed:
struck attempts (see `Archive::penalties`) first, otherwise in archive order,
each with a coarse `outcome` (`crashed`, `regressed`, `flat`, `cooled`,
`denied` or none). No delta, interval, patch body or rationale is sent. In
`Retreat` mode `evaluate_retreat_epoch` remembers `HARMFUL` candidates as
`harmful` and confidently regressed ones as `regressed`, with their public
patch shape; retreat memory recall then leads with an avoid line naming those
shapes. A pair already labelled `rejected` before these labels existed is
left as it is.
