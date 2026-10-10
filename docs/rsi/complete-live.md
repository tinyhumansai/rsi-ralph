# Complete local integration run — 2026-10-11

The bounded OpenRouter runs completed with **Inconclusive** experimental
evidence. A separate model-authored context repair passed normalized confirmation,
both sandbox gate decisions, a verified turn on its accepted SQLite deployment,
exact parent rollback, and reopening archive, deployment and runtime.

## Post-review replication

After fixing the four review findings, v8 froze a new protocol and checker
fingerprints before generation. Its [aggregate receipts](results/2026-10-11/post-review/)
are separate from the earlier v7 receipts below; no results are pooled.

The fresh retreat made **121 calls**, measured **158,098 generation tokens**,
and produced **50 typed occurrences** plus **30 invalid formatter occurrences**.
The funnel retained 17 WrongClass, 31 Ungrounded, one Guard rejection and one
NoGain rejection. The single screened candidate had H delta **−0.0001484**;
no candidate reached approval, and the baseline survived restart. Generation
reservation was **1,620,641** against the **3,000,000** cap.

The fresh comparison scheduled all **24 attempts**, retaining **20 verified
rejections and four failures**. Generation/evaluation reservations were
**2,572,658 / 2,448,280**; elapsed time was **376.52 seconds**. Full, MemoryOff
and RetreatOff had 4, 4 and 1 available pairs, with zero efficiency deltas and
no accepted anchors. The verdict remains **Inconclusive**.

The separate post-review controlled run used **645 measured generation tokens**
and again passed all three H stages (0.45 → 0.975), deny/accept gates, a real
verified SQLite deployment turn, exact rollback and durable reopening. Proof
credit remained zero. The retreat-store contamination canary scan found zero
matches; store counts and tool versions are in its execution metadata.

The sections below preserve the earlier v7 and controlled v2 results.

## Published earlier evidence

The [results directory](results/2026-10-11/) contains only public aggregate
receipts, frozen protocol, configuration provenance and execution metadata.
Private model outputs, task-level rows and raw runtime journals remain under
checkout `target/` and are not committed.

- [Frozen protocol](results/2026-10-11/preregistration.json)
- [Configuration provenance](results/2026-10-11/configuration-provenance.json)
- [Complete experiment](results/2026-10-11/experiment-report.json)
- [Worked retreat funnel](results/2026-10-11/worked-report.json)
- [Separate acceptance/deployment validation](results/2026-10-11/controlled-report.json)
- [Tool versions, wall time and contamination scan](results/2026-10-11/run-metadata.json)

## Actual model and verifier execution

The provider was `openai/gpt-4.1-mini`, using the local inherited OpenRouter key.
The proposal engine was the vendored `tinybackrooms::Engine`, with its actual
critic/proposer, native structured tools and durable creative memory. All model
calls reserved bounded input/output allowances before dispatch. Credentials and
private inputs use subprocess stdin rather than command arguments.

The code checker ran a pinned Python Docker image with network disabled,
read-only filesystem, dropped capabilities, an unprivileged user and bounded
resources. Its frozen grammar permits only a pure arithmetic `solve(x)` function;
forged stdout and early exit cannot establish execution credit. Lean 4.34.1 was
invoked as the actual checker. Stock-kernel smoke tests verified a valid proof
and rejected an admitted/forged-report artifact even when compilation exited zero.
CommandSpec itself is a checker port, not an OS sandbox; a production host owns
sandboxing for additional generated-program/tool capabilities.

## Worked retreat

The engine made **129 calls**, measured **159,278 generation tokens**, returned
**57 typed proposal occurrences** and counted **33 invalid formatter occurrences**.
Every typed occurrence reached the persisted funnel: **22 WrongClass** and
**35 Ungrounded**. No candidate reached paired evaluation or approval, so the
screen yield and confirmed acceptance share were zero and verified gain was
unavailable. The selected baseline survived durable restart. The generation
reservation was 1,746,558 against its 3,000,000 cap; reservations are upper bounds,
not measured provider charges. Discovery ranks did not override rejection.

## Preregistered comparison

The protocol froze before generation: four proposal seed draws, two paired anchor
seeds per draw, two proposer arms, and Full/MemoryOff/RetreatOff ablations, giving
**24 scheduled attempts**. Every assignment started from the same solver package,
empty learned hive memory and identical seeded public retreat history. MemoryOff
could not recall that history. Generation and evaluation caps were each 500,000
per assignment; screening, retries and initial/final anchors count toward them.

There were **19 verified rejected attempts and 5 failed assignments**, with all
failures retained and conservatively charged the complete reservations. The
aggregate ledger reported generation reservation **3,048,998** and evaluation
reservation **2,925,866**. These include unused failure reservations and are not
provider billing totals. Elapsed time from preregistration through the worked run
and comparison was **343.28 seconds**.

The available Full, MemoryOff and RetreatOff pair counts were 3, 4 and 1.
Their observed efficiency deltas and intervals were zero. No anchor gain or
accepted proposal was established. Missing assignments and failed gates made the
final verdict **Inconclusive**; there is no improvement or ignition claim.

## Separate acceptance/deployment validation

A new, independently bounded run froze a narrower proposal schema before the
model call: only the public context item, exact precondition, epoch references and
`normalized_h` prediction were permitted. No model output was rewritten.
This run is outside the comparison and does not replace any failed trial.

Its model-authored proposal used **641 measured generation tokens**. Screening,
selection and fresh confirmation each measured incumbent H **0.45**, candidate H
**0.975**, and paired delta/lower bound **0.525** under the frozen toy calibration.
The denied gate retained the baseline. A separate archive accepted the same
proposal under the user's context-only sandbox authorization, then deployed it
using the SQLite CoordinatorHost. A real deployed model turn passed its host
READY checker. Rollback restored the parent package and exact memory watermarks;
the deployment, archive and SQLite host reopened successfully.

This authorization is not a production human-review service. The actual scored
families improved code while reference/schema already passed; proof credit
remained zero. The kernel integration tests independently exercised a valid proof.

## Limits and reproducibility

These are deliberately impaired public demonstration fixtures, not a curated
undisclosed benchmark. Task IDs and seeds are disjoint, but code and schema prompt
semantics repeat across stages; proof/reference instances vary. Repeated seeds
on tiny deterministic tasks produce degenerate intervals. Family anchors are
fixed theoretical zero/one calibration, not empirically established references.

Both proposer configurations are human-authored and frozen. The protocol arm
called Evolved is a comparison label, not an accepted learned orchestrator.
The harness accepts independently produced frozen artifacts; this run does not
establish recursive learning. CortexDB, broader production factories and upstream
language-verifier declarations remain separate integration boundaries.

Earlier development attempts were retained locally; v6 was withdrawn before
completing comparison after verifier review. They are not pooled with v7.
Reproduction commands and environment variables are in the root README; choose
fresh directories so interrupted execution cannot silently rerun model work.

## Validation

All four cargo contract commands, strict rustdoc, default-feature tests, bundled
example, Rust 1.96.1 build, transport-free bus dependency check, cargo-deny and the
exact CI per-file coverage script passed. The suite contains 231 library tests,
3 public integration tests, 18 bus tests and 10 doc tests. Every measured source
file exceeded 90% line coverage. Seven Python checker/setup regressions passed; actual
Docker good/bad/forged artifacts and stock Lean checks also passed.
