# Public failure evidence

This module turns a running coordinator into the public evidence a proposal
model is allowed to see. It reads; it never writes to the coordinator, to
memory or to disk. Persistence of observations is done by
`DeploymentStore::observe` (`crates/rsi-ralph-host/src/host/`) or
`EvidenceJournal` (`crates/rsi-ralph-host/src/evidence_journal/`).

| File | Responsibility |
| --- | --- |
| `mod.rs` | `Observer` (cursor and snapshot) and `Dossier::build` |
| `types.rs` | `Observation`, `TranscriptReference`, `EpisodeObservation`, `TurnUsage`, `SeatUsage`, `VerifierOutcome`, `MemoryHealth`, `Dossier` |
| `test.rs` | Redaction, cursor advance, ordering and saturation |

## Observer

`Observer::snapshot` reads the coordinator's transcript after its cursor and
all retained episode statuses. It advances the cursor only after both reads
succeed. These APIs are separate reads, so concurrent coordinator changes can
make the episode snapshot newer than the transcript snapshot.

Returned observations retain only sequence references, seat and episode
identities, opening sequences and redacted phases. Message bodies, private
readers, destinations and failure reasons are discarded.

An `Observation` is two lists:

- `transcript`: `TranscriptReference { sequence, seat_id, episode_id }`;
- `episodes`: `EpisodeObservation { episode_id, opened_at, phase }`, where
  `phase` is `Open`, `AwaitingRelease`, `Settled` or `Failed`.

## Dossier

`Dossier::build` combines an observation with explicit host token measurements,
node outcomes and memory health counters. It sorts and deduplicates evidence,
aggregates costs by seat and saturates counters on overflow. Conflicting node
outcomes retain both open and failed evidence rather than guessing chronology.
The caller determines the epoch window and supplies each turn once. Dossiers
contain no task text, oracle answers or implicit memory transactions.

```rust,ignore
let dossier = Dossier::build(
    epoch,            // host epoch number, usually DeploymentCheckpoint::epoch
    &observation,     // from Observer::snapshot or DeploymentStore::observe
    &turn_usage,      // TurnUsage per measured turn (seat, tokens, wasted)
    &outcomes,        // VerifierOutcome { node_id, status } per checked node
    MemoryHealth::default(),
);
```

The resulting fields are `epoch`, `transcript_sequences`, `failed_episodes`,
`open_episodes`, `failed_nodes`, `open_nodes`, `seat_usage`, `wasted_turns`
and `memory_health`. The host copies `transcript_sequences` and the failure
nodes it wants addressed into the epoch (`Epoch::transcript_refs` and
`Epoch::failure_nodes`; the live example uses `failed_nodes`). A proposal
whose `transcript_refs` or `Prediction::nodes` fall outside those lists stops
as `Ungrounded`.
`prepare_retreat_epoch` also checks that the dossier agrees with the persisted
observation before calling any model.

## Operational constraints

- **Stop admission at epoch boundaries.** The transcript and episode reads
  are separate snapshots. Without quiescence an epoch can see an episode
  phase whose transcript rows fall into the next epoch.
- **Usage must be measured, not reported by the model.** `TurnUsage` comes
  from the runner (for example `ControlledRunnerFactory::usage`), never from
  model output.
- **Each turn once.** `Dossier::build` does not deduplicate turns across
  calls; supplying the same turn in two epochs counts it twice.
  `EvidenceJournal` exists to enforce exactly-once consumption.
- **`Observer` cursors are in memory.** A fresh `Observer` starts at the
  beginning of the transcript. `DeploymentStore::observe` persists its own
  cursor and filters already settled episodes; use it rather than a bare
  `Observer` when epochs must survive restarts.
