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
Schema-invalid outputs remain counters, never persisted raw artifacts.

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
