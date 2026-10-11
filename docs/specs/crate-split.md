# Crate split

**Status:** Accepted. **Owner:** TinyRSI maintainers.

Plan: [`../plans/crate-split.md`](../plans/crate-split.md). Decision record:
[ADR 0002](../adr/0002-split-crates-drop-bus.md).

## Problem

`crates/tinyrsi` is one crate of roughly 16k non-test lines in 23 modules.
Several separate concerns share it:

- the hive epoch path;
- the generic [script loop](script-loop.md) and its [trace](loop-trace.md);
- the statistics that decide a comparison;
- the model boundary;
- a TinyBus lineage module.

A change to how candidates are *scored* rebuilds and retests the hive host. A
reader cannot tell from the dependency graph which parts are reusable. Nothing
stops the scoring code from growing a dependency on the coordinator. The TinyBus
module, its contract crate and the cdylib release no longer serve a consumer.

The loop has three conceptual phases that should be separable and testable on
their own. Observability is a fourth concern:

1. **Score**: decide whether a candidate is closer to the goal than the
   incumbent. Goals can be vague, so a score may come from a measured metric, an
   executed checker, or a language model acting as a judge. There is no loss
   function.
2. **Evolve**: given the incumbent, the goal and what was rejected, propose the
   next candidates.
3. **Run**: the loop that alternates the two, keeps the ledger, and stops.
4. **Observe**: emit what the loop is doing, for the
   [visualizer](rsi-viz.md) and for external tracing.

## Goals and non-goals

- Goal: each phase is its own workspace crate. Each can be tested alone with
  `cargo test -p <crate>`. Dependencies point one way, with no cycles.
- Goal: scoring and evolution are *ports*: traits that the loop calls and that
  can be implemented without touching the loop.
- Goal: an LLM-as-judge scorer and a tinybackrooms-backed scorer, alongside the
  existing metric scorer and verifiers.
- Goal: a top-level `examples/` package that runs the whole system offline on
  three problems: a math problem, upgrading a file to a written standard, and
  improving a hive of agents.
- Goal: remove `crates/tinyrsi-bus`, the TinyBus module and its release
  packaging.
- Non-goal: changing any decision rule. `evaluation::compare` and
  `scoring::compare_h` stay the only accept/reject logic.
- Non-goal: changing any persisted format. Archive JSON, the script-loop ledger
  and the `events.jsonl` wire schema stay byte-identical.
- Non-goal: a new user interface. `apps/rsi-viz/` is the visualizer.
- Non-goal: refactoring the hive orchestrator onto the generic ports. The hive
  path keeps its staged funnel, and only gains trace emission.

## Proposed behavior

### Crates

Dependencies point upward in the table: a crate may depend only on crates
listed above it.

| Crate | Holds | Depends on |
|---|---|---|
| `tinyrsi-types` | `Proposal`, `Prediction`, `CandidateState`, `ReasonCode`, `EvalStage`, `ProposalEvent`, `Goal`; re-exports `Patch`, `Package`, `PatchClass`, `Operation`, `Record`, `VersionMetadata` from `tinyhivemind-lang` | tinyhivemind-lang |
| `tinyrsi-model` | `ModelPort`, `ModelRequest`, `ModelReply`, `OpenRouterModel`, `BudgetedModel`, `GenerationMeter`, `ModelConversant`, `block_on` | tinybackrooms |
| `tinyrsi-trace` | `LoopEvent`, `TraceRecord`, `TraceSink`, `JsonlTrace`, `VecTrace`, `NullTrace`, clocks; `TracingTrace`, `FanoutTrace` | tracing |
| `tinyrsi-langfuse` | `LangfuseTrace`, a `TraceSink` that exports runs to Langfuse | trace, openhuman-embed (`langfuse` only) |
| `tinyrsi-score` | `evaluation`, `scoring`, `experiment`; the `Scorer` port; `MetricScorer`, `LlmJudge`, `BackroomsJudge`, `Composite` | model, trace, tinybackrooms |
| `tinyrsi-verify` | `process`, `blueprint`, `verification` (including `lake`); `VerifierScorer` | score, tinyhivemind-hives |
| `tinyrsi-evolve` | `retreat`, `retreat_memory`; the `Evolver` port; `ScriptSupervisor`, `ScriptedEvolver`. `retreat` reads a narrow `RetreatDossier` view instead of `observation::Dossier`, which stays in core. | types, model, tinybackrooms |
| `tinyrsi` | `orchestrator`, `archive`, `lineage`, `memory`, `observation`, `script_loop` (driver, execution, ledger) | all of the above |
| `tinyrsi-host` | `host`, `suites`, `epoch_driver`, `evidence_journal` | tinyrsi, tinyhivemind-hives (sqlite), tokio |
| `tinyrsi-examples` | three runnable examples, `publish = false` | tinyrsi-host |

`tinyrsi-host` replaces the `host` Cargo feature: its modules are exactly the
ones the feature gated. `memory` and `observation` compile without the feature
today, so they stay in `tinyrsi` and keep their paths; only the
`MemorySnapshot` implementation for `SnapshotMemoryStore`, which already
needed the feature, moves to `tinyrsi-host` beside the trait.

Each crate has its own `Error` and `Result`. A crate converts the errors of the
crates it depends on into the variants its callers already match. `tinyrsi`
re-exports every crate below it, so existing `tinyrsi::Name` paths keep
resolving for everything that was not behind the `host` feature.

**Host-feature paths do break.** `tinyrsi` cannot re-export `tinyrsi-host`,
because the host crate depends on it. Items that were reachable only with
`--features host` therefore move to `tinyrsi_host::Name`:
`CoordinatorHost`, `DeploymentStore`, `MemorySnapshot`,
`CoordinatorEvaluator`, `EvidenceJournal`, the `epoch_driver` items and the
rest. A consumer replaces `tinyrsi = { features = ["host"] }` with a
dependency on `tinyrsi-host` and changes those imports.
`tinyrsi-host` re-exports `tinyrsi`, so after that one change a single import
root covers both. The release notes for the version that drops the feature
must list this migration.

### Scorer port

```rust
pub trait Scorer<S: ?Sized> {
    /// One assessment of `subject` against `goal`, one quality per seed.
    fn assess(&self, subject: &S, goal: &Goal, seeds: &[u64]) -> Result<Assessment>;
}

pub struct Assessment {
    pub qualities: Vec<f64>,          // aligned with `seeds`, each in (0, 1)
    pub eligible: Option<Vec<f64>>,   // qualities from accept-eligible evidence only
    pub metric: Option<MetricEvidence>, // raw per-seed metric and direction, if any
    pub rationale: Option<String>,    // a judge's reasoning, when there is one
    pub tokens: u64,                  // measured model tokens spent assessing
}
```

The port is synchronous, like the script loop. Model-backed scorers drive their
`ModelPort` future on the calling thread with `tinyrsi_model::block_on`.
`decide(incumbent, candidate, seeds, policy)` turns two assessments into a
`ScoreDecision` and an interval, using `evaluation::compare`. It is the only
decision function, and it applies three rules:

1. The decision is `compare`'s on `qualities`.
2. **Eligibility gate.** An `Accept` stands only if `compare` also accepts on
   `eligible`. When either side's `eligible` is `None`, the decision is capped
   at `Inconclusive`. Evidence that cannot support acceptance on its own
   therefore never decides one, whatever its weight.
3. **Units.** When both sides carry `metric`, the reported estimate and bounds
   are the mean paired metric improvement `s * (candidate - incumbent)` plus
   or minus the policy's standard-error multiple, in metric units. This is
   exactly what `script_loop/score.rs` reports today. Otherwise they are in
   quality units.

Implementations:

- `MetricScorer`: the script loop's existing quality map
  `q = 1/2 + atan(s * m) / pi` over measured per-seed metrics. It sets
  `eligible` to its qualities and carries `metric`.
- `LlmJudge`: a rubric prompt, `raters` independent forced tool calls per seed
  and a median quality. It is constructed with an optional `JudgeCalibration`.
  It sets `eligible` only when that calibration passes its threshold. An
  uncalibrated judge still assesses, but by rule 2 it can never produce an
  `Accept` on its own.
- `BackroomsJudge`: tinybackrooms' consensual-assessment judge, aimed at a
  goal. N independent raters score goal fit from 1 to 7 (optionally times
  novelty, tinybackrooms' quality gate) through any tinybackrooms
  `Conversant`, and their mean maps into `[0, 1]`. tinybackrooms' own scorer
  is private, so this reuses its method and backends rather than its code.
  It carries the same calibration gate as `LlmJudge`.
- `Composite`: a weighted mean of members' per-seed qualities. Weights are
  positive and sum to one. Its `eligible` is the weighted mean of the
  *eligible members only*, with their weights renormalized, or `None` when no
  member is eligible. It carries `metric` only when it has a single member
  that carries one. A blend's decision is driven by blended qualities, and a
  metric interval reported beside it could contradict that decision (an
  `Accept` beside a negative metric interval). So a blend reports its
  interval in quality units. A high-weight uncalibrated judge can widen the
  margin, but the eligible evidence must still show an improvement by itself.
- `VerifierScorer` (in `tinyrsi-verify`): a `HostVerifier` or Lake check per
  seed. A pass is quality `1 - ε` and a failure is `ε`.

### Evolver port

```rust
pub trait Evolver<S: ?Sized> {
    /// What this evolver proposes: a change to apply, not a new subject.
    type Proposal;
    fn propose(&self, incumbent: &S, goal: &Goal, context: &EvolveContext<'_>)
        -> Result<Proposed<Self::Proposal>>;
}

pub struct Proposed<P> {
    pub tokens: u64,                         // measured model tokens spent
    pub proposals: Vec<Result<P, Rejection>>, // in the evolver's rank order
}
```

An evolver proposes *changes*; it does not build candidates. Its proposals
have their own type, because what an evolver reads is not what it writes.
The script supervisor reads a `ScriptSubject` and writes a `ScriptProposal`
(files to replace). Retreat reads a `Package` and writes ranked `Proposal`s
(typed patches with predictions). Turning a proposal into a candidate
(materializing files, applying a patch, pinning memory) stays with the loop
that owns the subject, as it does today.

`proposals` keeps the evolver's order, so retreat's ranking survives, and
each proposal carries its own metadata (rationale, hypothesis, prediction).
A `Rejection` is a proposal the evolver produced but could not validate (a
malformed reply, a path outside the editable set). The loop records it and
continues past it, which is the script loop's current behavior. A transport
failure is the outer `Err` and ends the run.

`EvolveContext` carries the epoch, the proposal index and seed, the score
history, and the last `rejected_context` rejected candidates with their
decision and notes. Implementations:

- `ScriptSupervisor`: `Evolver<ScriptSubject, Proposal = ScriptProposal>`,
  today's `Supervisor`, unchanged in prompt and tool schema. It yields one
  proposal per call.
- `RetreatEvolver`: `Evolver<Package, Proposal = Proposal>`, wrapping
  `generate_retreat`. It yields the ranked list.
- `ScriptedEvolver<S, P>`: replays canned proposals, for tests and offline
  examples.

### Observe

`tinyrsi-trace` owns the `events.jsonl` schema unchanged. It adds
`TracingTrace`, a `TraceSink` that mirrors each `LoopEvent` as a `tracing`
event inside per-run and per-epoch spans, and `FanoutTrace`, which writes one
stream to several sinks.

Langfuse export reuses OpenHuman's existing client and span builder rather
than a second exporter stack. `tinyrsi-langfuse` depends on `openhuman-embed`
with only its `langfuse` feature and provides `LangfuseTrace`.

- **Mapping.** Each run is one trace with a root `tinyrsi.run` span. Each
  epoch is a child span, and each stage is a zero-length child span carrying
  the event's wire JSON. The run's output includes the final incumbent metric.
- **Transport.** It uses OTLP, Langfuse's supported trace path (the legacy
  batch ingestion API is sunset on Langfuse Cloud on 2026-11-16). When a run
  finishes, OpenHuman's `otlp_requests` builds the requests under its span and
  byte caps, and `LangfuseClient::send_otlp` posts them to
  `/api/public/otel/v1/traces`.
- **Runtime.** A worker thread owns the transport's Tokio runtime, so the sink
  is safe to use and to drop inside another runtime.
- **Configuration** is OpenHuman's: `LANGFUSE_BASE_URL`, `LANGFUSE_PUBLIC_KEY`
  and `LANGFUSE_SECRET_KEY`, or the TinyHumans proxy.
- **Failures.** Export is best-effort, because the trace is observability: a
  failed export is counted and retried on the next flush, and never returned
  to the loop.

It is its own crate so hosts that need only `events.jsonl` do not compile
OpenHuman. The hive `epoch_driver` emits the existing event
kinds at its stage boundaries, so `apps/rsi-viz/` shows hive epochs live.

### Removed

- `crates/tinyrsi-bus`.
- `src/tinybus_module/`.
- The `verify_module` and `verify_github_release` examples.
- `MODULE.md`.
- The `vendor/tinybus` submodule.
- The cdylib crate type.
- The release workflow's module packaging, checksum and install jobs.

`GradeEntry` and the lineage `Verdict` move into `tinyrsi::lineage`. The
crate-root `append` and `head` wrappers, which only adapted bus payloads, are
removed; `Lineage::append` and `Lineage::head` remain.

## Invariants and constraints

- No crate depends on a crate below it in the table. `tinyrsi-score`,
  `tinyrsi-evolve` and `tinyrsi-trace` never depend on `tinyhivemind-hives`,
  SQLite or Tokio.
- `evaluation::compare` and `scoring::compare_h` are the only accept/reject
  rules. A scorer produces qualities and never decides.
- A judge without passing calibration can never be the reason a candidate is
  accepted: `decide` requires acceptance on accept-eligible evidence alone.
- Archive JSON, `ledger.json` and `events.jsonl` keep their exact bytes for the
  same inputs. The existing pinned tests stay unchanged and green.
- Every public item keeps a rustdoc comment. Every source file keeps at least
  90% line coverage.

## Acceptance criteria

- `cargo test -p <crate>` passes alone for every crate in the table.
- `cargo tree -p tinyrsi-score`, `-p tinyrsi-evolve` and `-p tinyrsi-trace`
  contain neither `tinyrsi` nor `tinyhivemind-hives`.
- The script loop runs on `Scorer` and `Evolver`. Its existing tests pass
  unchanged, apart from constructing a `ScriptSupervisor` where they built a
  `Supervisor`.
- Tests show an uncalibrated `LlmJudge` alone yields no `Accept`. They also
  show that a `Composite` of a high-weight favourable uncalibrated judge and a
  low-weight flat eligible scorer yields no `Accept`.
- `decide` on `MetricScorer` assessments reproduces the script loop's current
  decisions and metric-unit intervals exactly.
- `cargo run -p tinyrsi-examples --example math_problem`, `upgrade_file` and
  `hive_goal` run offline, each records at least one accepted and one rejected
  candidate, and each writes an `events.jsonl` that `apps/rsi-viz/` lists.
- Nothing in the workspace references `tinybus` or `tinyrsi_bus`.

## Open questions

None.
