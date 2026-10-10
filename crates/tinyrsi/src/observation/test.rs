//! Read-only cursor safety, privacy and deterministic aggregation.
#![allow(clippy::unwrap_used)]
use super::*;
use tinyhivemind_hives::{Destination, EpisodeStatus, Message};
fn message() -> Message {
    Message {
        message_id: "private".into(),
        sequence: 7,
        sender: "seat".into(),
        destination: Destination::Agent("private recipient".into()),
        body: "private task secret".into(),
        thread: None,
        episode_id: None,
        only_for: vec!["private reader".into()],
    }
}
#[test]
fn failed_reads_do_not_advance_the_cursor_and_private_payloads_are_discarded() {
    let observer = Observer::new();
    assert_eq!(
        observer
            .read_snapshot(|| Err(Error::ObservationFailed), || Ok(vec![]))
            .unwrap_err(),
        Error::ObservationFailed
    );
    assert_eq!(
        observer
            .read_snapshot(|| Ok(vec![message()]), || Err(Error::ObservationFailed))
            .unwrap_err(),
        Error::ObservationFailed
    );
    assert_eq!(observer.cursor(), None);
    let (snapshot, cursor) = observer
        .read_snapshot(
            || Ok(vec![message()]),
            || {
                Ok(vec![EpisodeStatus {
                    episode_id: "episode".into(),
                    hive_id: "hive".into(),
                    opened_at: 7,
                    thread: None,
                    starters: vec![],
                    phase: EpisodePhase::Failed("private oracle".into()),
                }])
            },
        )
        .unwrap();
    assert_eq!(cursor, Some(7));
    assert_eq!(snapshot.episodes[0].phase, ObservedEpisodePhase::Failed);
    let public = serde_json::to_string(&snapshot).unwrap();
    assert!(!public.contains("private"));
    assert_eq!(
        observer
            .read_snapshot(|| Ok(vec![]), || Ok(vec![]))
            .unwrap()
            .1,
        None
    );
}
#[tokio::test]
async fn coordinator_snapshot_is_read_only_and_preserves_cursor_when_empty() {
    use std::sync::Arc;
    use tinyhivemind_hives::{CoordinatorOptions, MemoryStorage};
    let coordinator = Coordinator::new(
        "observer-test".into(),
        Arc::new(MemoryStorage::new()),
        CoordinatorOptions::default(),
    )
    .await
    .unwrap();
    let mut revisions = coordinator.subscribe();
    revisions.borrow_and_update();
    let mut observer = Observer::new();
    assert_eq!(
        observer.snapshot(&coordinator).unwrap(),
        Observation::default()
    );
    assert!(!revisions.has_changed().unwrap());
    observer.cursor = Some(7);
    observer.snapshot(&coordinator).unwrap();
    assert_eq!(observer.cursor(), Some(7));
}
#[test]
fn dossier_orders_evidence_and_aggregates_host_cost_without_overflow() {
    let observation = Observation {
        transcript: vec![
            TranscriptReference {
                sequence: 7,
                seat_id: "seat".into(),
                episode_id: None
            };
            2
        ],
        episodes: vec![
            EpisodeObservation {
                episode_id: "failed".into(),
                opened_at: 1,
                phase: ObservedEpisodePhase::Failed,
            },
            EpisodeObservation {
                episode_id: "waiting".into(),
                opened_at: 2,
                phase: ObservedEpisodePhase::AwaitingRelease,
            },
            EpisodeObservation {
                episode_id: "open".into(),
                opened_at: 3,
                phase: ObservedEpisodePhase::Open,
            },
            EpisodeObservation {
                episode_id: "done".into(),
                opened_at: 4,
                phase: ObservedEpisodePhase::Settled,
            },
        ],
    };
    let mut turns = vec![
        TurnUsage {
            seat_id: "z".into(),
            input_tokens: u64::MAX,
            output_tokens: 5,
            wasted: true,
        },
        TurnUsage {
            seat_id: "z".into(),
            input_tokens: 1,
            output_tokens: 6,
            wasted: false,
        },
        TurnUsage {
            seat_id: "a".into(),
            input_tokens: 1,
            output_tokens: 2,
            wasted: true,
        },
    ];
    let mut outcomes = vec![
        VerifierOutcome {
            node_id: "f".into(),
            status: NodeStatus::Failed,
        },
        VerifierOutcome {
            node_id: "o".into(),
            status: NodeStatus::Open,
        },
        VerifierOutcome {
            node_id: "s".into(),
            status: NodeStatus::Sorry,
        },
        VerifierOutcome {
            node_id: "v".into(),
            status: NodeStatus::Verified,
        },
    ];
    let memory = MemoryHealth {
        retrieval_failures: 1,
        write_failures: 2,
        stale_entries: 3,
        repeated_failed_attempts: 4,
        unused_recalls: 5,
        clipped_recalls: 6,
        contradictory_entries: 7,
    };
    let dossier = Dossier::build(3, &observation, &turns, &outcomes, memory);
    turns.reverse();
    outcomes.reverse();
    assert_eq!(
        dossier,
        Dossier::build(3, &observation, &turns, &outcomes, memory)
    );
    assert_eq!(dossier.transcript_sequences, [7]);
    assert_eq!(dossier.failed_episodes, ["failed"]);
    assert_eq!(dossier.open_episodes, ["open", "waiting"]);
    assert_eq!(dossier.failed_nodes, ["f"]);
    assert_eq!(dossier.open_nodes, ["o", "s"]);
    assert_eq!(dossier.seat_usage["z"].input_tokens, u64::MAX);
    assert_eq!(dossier.seat_usage["z"].output_tokens, 11);
    assert_eq!(dossier.wasted_turns, 2);
    assert_eq!(dossier.memory_health, memory);
}

#[test]
fn snapshot_redacts_all_episode_phases_and_orders_references() {
    let observer = Observer { cursor: Some(4) };
    let mut earlier = message();
    earlier.sequence = 5;
    let phases = [
        EpisodePhase::Open,
        EpisodePhase::AwaitingRelease,
        EpisodePhase::Settled,
    ];
    let statuses = phases
        .into_iter()
        .enumerate()
        .map(|(index, phase)| EpisodeStatus {
            episode_id: index.to_string(),
            hive_id: "hive".into(),
            opened_at: 5,
            thread: None,
            starters: vec![],
            phase,
        })
        .rev()
        .collect();
    let (observation, cursor) = observer
        .read_snapshot(|| Ok(vec![message(), earlier]), || Ok(statuses))
        .unwrap();
    assert_eq!(cursor, Some(7));
    assert_eq!(
        observation
            .transcript
            .iter()
            .map(|row| row.sequence)
            .collect::<Vec<_>>(),
        [5, 7]
    );
    assert_eq!(
        observation
            .episodes
            .iter()
            .map(|episode| episode.phase)
            .collect::<Vec<_>>(),
        [
            ObservedEpisodePhase::Open,
            ObservedEpisodePhase::AwaitingRelease,
            ObservedEpisodePhase::Settled
        ]
    );
    assert_eq!(
        observer
            .read_snapshot(|| Ok(vec![]), || Ok(vec![]))
            .unwrap()
            .1,
        Some(4)
    );
}

#[tokio::test]
async fn coordinator_cursor_consumes_each_private_payload_once() {
    use std::sync::Arc;
    use tinyhivemind_hives::{
        AgentRegistration, AgentRunner, CoordinatorOptions, MemoryStorage, SendMessage, TurnFuture,
        TurnRequest,
    };
    struct UnusedRunner;
    impl AgentRunner for UnusedRunner {
        fn run(&self, _: TurnRequest) -> TurnFuture {
            Box::pin(async { Err(tinyhivemind_hives::Error::InvalidOptions) })
        }
    }
    let coordinator = Coordinator::new(
        "cursor-test".into(),
        Arc::new(MemoryStorage::new()),
        CoordinatorOptions::default(),
    )
    .await
    .unwrap();
    coordinator
        .register_agent(AgentRegistration {
            agent_id: "seat".into(),
            runtime_id: "cursor-test".into(),
            runner: Arc::new(UnusedRunner),
        })
        .await
        .unwrap();
    let receipt = coordinator
        .send_as_host(SendMessage {
            message_id: "task".into(),
            sender: String::new(),
            destination: Destination::Agent("seat".into()),
            body: "private task secret".into(),
            thread: None,
            only_for: vec![],
            starters: vec![],
        })
        .await
        .unwrap();
    let mut revisions = coordinator.subscribe();
    revisions.borrow_and_update();
    let mut observer = Observer::default();
    let snapshot = observer.snapshot(&coordinator).unwrap();
    assert_eq!(snapshot.transcript.len(), 1);
    assert_eq!(observer.cursor(), Some(receipt.sequence));
    assert!(
        !serde_json::to_string(&snapshot)
            .unwrap()
            .contains("private task secret")
    );
    assert_eq!(observer.snapshot(&coordinator).unwrap().transcript, []);
    assert!(!revisions.has_changed().unwrap());
}
