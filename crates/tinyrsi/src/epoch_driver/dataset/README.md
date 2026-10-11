# Selection-only proposal outcomes

`EpochJournal::proposal_dataset` projects a prepared epoch and its validated
archive into `ProposalOutcomeDataset`. It retains every typed occurrence,
formatter call/item offset and hashed formatter-run identity, including exact
copies. The source arm, frozen input fingerprint, generation seed and public
`ModelPort::identity` describe provenance. Unknown or legacy identities stay
absent. `OpenRouterModel` attests its host-configured model identifier; this does
not attest a remote provider's implementation or mutable alias resolution.

Rows include the committed normalized-H minimum-gain prediction, ranking status,
observed stage, fixed host reason and candidate identity. Only selection-stage
H gain, paired bounds and eligibility enter labels and metrics. Confirmation
and anchor numbers, private task draws, generated prose, edits, tool output and
free-form error reasons are excluded. Stage presence and final approval/rejection
remain visible, but approval denial never changes a positive selection label.

Call `register_proposal_priors(epoch, probabilities, archive)` after generation
and before evaluation if calibrated probability forecasts are available. It
requires one optional probability per typed occurrence, validates [0, 1] and
persists the complete vector atomically. Changed or retroactive registration is
rejected; identical replay is harmless. A minimum-gain claim is not a probability,
so absent probabilities remain absent rather than becoming synthetic priors.
The host remains responsible for empirical calibration and training provenance.

`dataset.metrics(k)` reports Brier score and its complete-pair count, tied-rank
Spearman correlation on selection gain, and precision at k in predicted-gain
order. Constant rank vectors produce no correlation. Equal gains use proposal
order for top-k ties. Missing/censored rows participate in ranking; if any top-k
label is unresolved, precision is absent. Missing-prior and missing-label counts
make selection bias visible. These descriptive complete-case metrics do not
correct for nonrandom missingness or establish improvement.

Duplicated proposals never inherit another occurrence's measurements. Rows
skipped after acceptance are censored; preselection stops lack a selection label.
An interrupted transaction can expose its durable selection evidence while
retaining the interruption flag and absent final reason. Legacy journals lack
frozen selection bindings and therefore expose no numeric selection labels.
Failed generation retains provenance and accounting with no typed rows.
`generation_complete = false` marks incomplete accounting after interruption;
its initial metered counters must not be treated as final zero spending.
Projection rejects missing candidate records and conflicting frozen policy,
budget or suite bindings rather than silently taking evidence from another run.

After stopping writers, report an existing host run without repeating calls:

```sh
cargo run -p tinyrsi --features host --example proposal_metrics -- \
  path/to/epoch-journal.json path/to/archive 1 5
```

The journal and archive open with their usual exclusive locks. Keep generation
sidecars together with the journal. JSON output contains the safe dataset and its
metrics; private storage contents remain in those files. Empty generations have no valid nonzero top-k. The example still reports their
generation receipts, provenance and accounting, with `metrics: null`.
