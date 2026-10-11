# Generation occurrence receipts

Related: issue #6 and the scoring audit in issue #1.

Before every `submit_items` call, persist a call intent tied to the frozen epoch
fingerprint and generation arm. Before exposing its response to the Engine,
persist one safe disposition per returned item, preserving duplicate occurrences.
Classify schema, strict decoding and public-text guard failures separately.
Failed calls and malformed envelopes receive call-level dispositions. Interrupted
calls remain explicitly pending; reopening never retries them.

Receipts contain only host-assigned consecutive IDs, item offsets, generation mode,
frozen fingerprint and fixed disposition codes. They contain no generated text,
private labels, prompts, raw errors or output digests. A versioned sidecar shares
the epoch journal's exclusive writer lifetime and uses atomic fsynced replacement.
Legacy journals without a sidecar report unavailable evidence rather than invented
receipts. This supplies audit infrastructure; it does not establish an ablation win
or replace the later host evaluation classifications.
