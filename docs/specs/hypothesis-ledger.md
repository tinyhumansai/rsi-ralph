# Hypothesis ledger and diverse slots

**Status:** Accepted. **Owner:** TinyRSI maintainers.

Related: [script loop](script-loop.md), [loop trace](loop-trace.md),
[roadmap items 1 and 2](../rsi/script-loop-roadmap.md),
[plan](../plans/hypothesis-ledger.md).

## Problem

Live runs of the [script loop](script-loop.md) spend most of their proposal
budget on ideas the run has already ruled out. In the
[diagnostics reruns](../rsi/diagnostics-results.md) `gpt-4.1-mini` proposed
"greedy recoloring after DSATUR" six times although its own diagnostics said
the pass made 0 moves. The prompt shows only the last few rejected proposals
as free text, so an idea that fell out of that window is invisible, and two
rewordings of the same idea look unrelated.

Within one epoch the slots are asked the same question: the same prompt at
temperature 0. Two or three slots often return identical proposals, and the
second is discarded as a duplicate after a full model call.

## Goals and non-goals

- Goal: every proposal names its technique, and the prompt shows a compact,
  deduplicated list of the techniques already tried with their outcome.
- Goal: the supervisor is told not to resubmit a rejected technique unless it
  states what is concretely different, and the proposal has a field for that.
- Goal: each slot of an epoch gets its own instruction, sees what earlier slots
  of the same epoch proposed, and may sample at its own temperature.
- Goal: ledgers, traces and canned replies written before this change still
  load; the trace change is additive.
- Non-goal: refusing a resubmitted approach without running it. Labels are
  free text and normalization is lexical, so a hard refusal would also block
  genuinely new work under a similar name. The benchmark harness
  (roadmap item 6) should measure the soft rule first.
- Non-goal: semantic clustering of approaches (embeddings, a model call).
- Non-goal: recording sampling parameters in `ModelIdentity`; it names the
  provider and model only, and no receipt records sampling today.

## Proposed behavior

### Proposal fields

`ScriptProposal` (the `propose_script` tool arguments) gains:

- `approach: String`: a short label for the technique, such as
  `"tabu search"`, `"DSATUR"` or `"cubic least squares"`. The tool schema
  **requires** it with `maxLength: 60`, so a live model always sends one.
  Serde defaults it to `""`, so a canned reply or stored proposal written
  before this change still decodes; an empty label means "unlabelled".
- `differs_from_previous: Option<String>`: what is concretely different from
  an earlier attempt of the same approach. Optional in the schema, `null` or
  absent when the approach is new.

`ScriptProposal::approach_label()` returns the label with its whitespace
collapsed to single spaces and cut to 60 characters (an ellipsis replaces the
rest), or `None` when it is blank. Over-long labels are cut, not refused: a
proposal costs a full model call and its code may still be good.

`differs_from_previous` is a separate field rather than a sentence in the
rationale because the loop can then show it next to the approach, record it in
the ledger, and the benchmark harness can count resubmissions that state a
difference against ones that do not. Folding it into the rationale would make
both unmeasurable.

### Ledger

`LedgerCandidate` gains three optional fields, each omitted from
`ledger.json` when absent so older ledgers keep their bytes and still load:

- `approach`: the proposal's `approach_label()`.
- `differs_from_previous`: the proposal's statement, trimmed, when non-empty.
- `temperature`: the sampling temperature the loop requested for the slot,
  when the slot set one. A supervisor may ignore it (offline replays do), so
  it is a request, not a guarantee.

### Approaches already tried

Before each ideation call the loop groups the ledger's proposal rows (the
baseline and rows without a label are skipped) by **normalized** label:
lowercased, every run of characters that are not letters or digits replaced by
one space, trimmed. `"Tabu-Search"`, `"tabu search"` and `" TABU  search! "`
are one approach. Each group reports:

- the most recent raw label, shown in the prompt;
- the number of attempts;
- the best mean metric over its measured attempts, in the subject's direction;
- the last attempt's decision and epoch;
- a one-line verdict of the last attempt: its first failed seed's error, else
  its note (duplicate, scorer error), else the last `DIAG` line of its first
  seed with diagnostics, else `no diagnostics`, cut to 160 characters;
- the last attempt's `differs_from_previous`, when it gave one.

Groups are ordered by their most recent attempt, newest first, and at most
`LoopPolicy::approach_context` (default 12) are shown. The prompt renders them
as `# Approaches already tried` directly above `# Recently rejected
proposals`, followed by the rule:

> Do not resubmit an approach whose last decision was not accept unless your
> proposal's `differs_from_previous` states concretely what is different this
> time.

`approach_context: 0` removes the section and the rule.

### Earlier proposals this epoch

The loop ideates and executes one slot at a time, so slot `k` already sees the
measured outcome of slots `0..k` through the ledger. An accepted candidate of
the current epoch is not promoted until the epoch ends, so it appears in no
other section. The prompt therefore also lists every earlier slot of the
current epoch under `# Earlier proposals this epoch`: proposal id, approach
(or `unlabelled`), decision and mean. The section is omitted for slot 0.

### Slot directives

`LoopPolicy` gains `slots: Vec<SlotDirective>`; a `SlotDirective` is
`{ instruction: String, temperature: Option<f64> }`. Slot `k` of an epoch uses
`slots[k % slots.len()]`. Its instruction is rendered last in the prompt,
under `# This proposal (slot k+1 of K)`, and its temperature goes into
`ScriptBrief::temperature`. An empty list renders no directive section and
requests no temperature, which reproduces the previous prompt structure.

The default list holds three directives, all without a temperature, so the
default sampling is unchanged:

1. best next step: the single change most likely to improve the metric now;
2. explore: a different family of approach from everything already tried
   and from the earlier proposals this epoch;
3. repair: take the most promising rejected approach and fix what its
   diagnostics show went wrong, saying what changed in
   `differs_from_previous`; with nothing rejected yet, the best next step.

A policy whose directive has a blank instruction, or a temperature that is not
finite or outside `[0, 2]`, is refused with `InvalidScriptSubject` before
anything is written.

### Temperature on the model path

`ModelRequest` gains `with_temperature(t) -> Result<Self>` and `temperature()`.
The default is `0.0`, so every existing request is unchanged. A temperature
that is not finite or outside `[0, 2]` is `InvalidRequest`. `OpenRouterModel`
sends the request's temperature instead of the constant `0`.
`ScriptSupervisor` applies `ScriptBrief::temperature` when it is set.

### Trace

`ideated` gains `approach: string`, the proposal's `approach_label()`. It is
**omitted** when the proposal has no label or was refused before
materialization, so traces of unlabelled runs, and of the hive loop, keep their
exact bytes. Readers treat a missing field and `null` alike. `apps/rsi-viz`
validates the field when present and shows it in the proposals feed.

## Invariants and constraints

- Adding the new fields changes no decision: scoring, promotion, duplicate
  detection and stop reasons are exactly as in the [script loop](script-loop.md).
- The approaches section is bounded by `approach_context` rows of at most a
  60-character label and a 160-character verdict, so it cannot grow with the
  run.
- Ledgers, traces and `propose_script` replies written before this change
  deserialize unchanged.
- The default policy requests no temperature; `OpenRouterModel` sends `0`
  unless a request says otherwise.

## Acceptance criteria

- Unit tests pin normalization, grouping, the best metric in both directions,
  the verdict precedence, recency ordering and the bound.
- A prompt test shows `# Approaches already tried` above the rejections, the
  resubmission rule, the earlier-slot section and the slot directive; a test
  with an empty slot list and `approach_context: 0` shows none of them.
- A loop test sees slot 1's prompt list slot 0's approach and decision, and
  the per-slot temperature reach the model request.
- Serde tests pin `ScriptProposal` with and without `approach`,
  `LedgerCandidate` with and without the new fields, and `ideated` with and
  without `approach`.
- Invalid temperatures (request and policy) and blank instructions are
  refused by tests.
- Every offline example still reaches its goal, with canned replies that name
  their approach.

## Open questions

None.
