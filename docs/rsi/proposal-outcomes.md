# Proposal outcome receipts

The host journal now projects a versioned dataset with formatter call/item
identities, hashed host run identities, configured model identity where known,
generation seed, predicted gain, frozen probability priors, observed stages and
final reasons. Each proposal occurrence keeps its own row, including duplicates.
Only the selection draw contributes measured gain and eligibility labels.

[Implementation contract](../specs/proposal-outcomes.md) and
[API and limitations](../../crates/rsi-ralph-host/src/epoch_driver/dataset/README.md)
describe binding, missingness and privacy rules.

## Offline protocol fixture

[Recorded report](results/2026-10-11/p4-outcomes-protocol-only.json) comes from the
`accepted_candidates_censor_later_occurrences_and_denials_do_not_label_duplicates`
regression fixture. The fixture uses scripted generation and evaluator results,
not a provider or private research benchmark. Its registered probability 0.8 is
an illustrative host prediction, not a calibrated or human-rated judgment.

One occurrence has a resolved selection label. Two duplicate occurrences have
no inherited label. The Brier score is approximately 0.04 over one pair;
Spearman correlation and precision at three are absent. These values verify the
reporting protocol and do not demonstrate calibration or improvement.

The example reads existing private files without repeating model or checker
calls:

```sh
cargo run -p rsi-ralph-host --example proposal_metrics --   target/outcome-demo/journal.json target/outcome-demo/archive.json 1 3
```

Private journal/archive files stay outside published evidence. Legacy unknown
provenance stays absent. Interrupted generation marks accounting as incomplete,
rather than representing initial zero counters as final measured spending.

Empirical calibration, informative generation ablations and independently curated
held-out experiments remain outstanding under issues #1, #4 and #6.
