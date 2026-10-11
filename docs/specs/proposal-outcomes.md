# Selection-only proposal outcome dataset

Related: issues #1 and #6 and the scoring acceptance map.

Publish versioned safe rows derived from prepared epoch proposals and validated
archive events. Preserve each typed proposal occurrence, including duplicates,
its formatter call/item identity where recorded, generation arm, host model
identity and seed where known, immutable input fingerprint, predicted normalized-H
minimum gain, stage reached, final fixed reason and selection-stage gain/interval.
Never export generated prose, patches, task/seed draws, oracle data, free-form
checker reasons or confirmation/anchor measurements. Legacy provenance is absent;
no identity, probability or missing outcome is invented.

Freeze optional probability-of-selection-eligibility priors before evaluation.
Do not interpret minimum gain as a probability. Reject later prior changes or
first registration after archive evaluation has started, including external runs.
A duplicate is a separate occurrence without an inherited selection label. Missing,
screen-stopped, interrupted, stopped-after-acceptance and unresolved selection
labels remain explicit. Archive evidence must match epoch, parent, proposal and
candidate identity; an unrelated archive is an error, not a source of zero labels.

Compute Brier score only from registered probabilities and resolved selection
labels; report its denominator. Spearman uses predicted gain and measured
selection gain with average ranks for ties and an absent result for a constant
rank vector. Precision at k ranks all occurrences by predicted gain, breaks ties
by proposal order and is absent if any top-k label is missing or unresolved;
never drop censored rows before ranking. Report missing-prior/label counts and
rank/precision denominators. These are descriptive metrics, not a calibration
claim or an informative ablation result.
