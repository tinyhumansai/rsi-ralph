//! Multi-seat dependency completion on the actual coordinator.
use super::*;
#[tokio::test]
async fn multi_seat_dependencies_complete_only_after_host_releases_verified_seat() {
    use std::sync::Arc;
    use tinyhivemind_hives::{
        AgentRegistration, Coordinator, CoordinatorOptions, Destination, EpisodePhase, HiveInfo,
        MemoryStorage, SendMessage,
    };
    let c = Arc::new(
        Coordinator::new(
            "runtime".into(),
            Arc::new(MemoryStorage::new()),
            CoordinatorOptions {
                round_width: 2,
                ..CoordinatorOptions::default()
            },
        )
        .await
        .unwrap(),
    );
    let make = |id: &str, seat: &str, deps: Vec<String>| crate::BlueprintNode {
        id: id.into(),
        statement: "answer".into(),
        owner_role: seat.into(),
        dependencies: deps,
        verifier: crate::Verifier {
            kind: crate::VerifierKind::HeldOutReference,
            spec: "golden".into(),
        },
        status: crate::NodeStatus::Open,
    };
    let blueprint = crate::Blueprint::new(vec![
        make("first", "seat", vec![]),
        make("second", "other", vec!["first".into()]),
    ])
    .unwrap();
    let registry = two_seat_registry();
    let runner = Arc::new(
        VerifiedRunner::new(
            &c,
            Arc::new(Producer {
                content: b"yes".to_vec(),
                forge: false,
            }),
            Arc::new(registry),
            blueprint,
        )
        .unwrap(),
    );
    for seat in ["seat", "other"] {
        c.register_agent(AgentRegistration {
            agent_id: seat.into(),
            runtime_id: "runtime".into(),
            runner: runner.clone(),
        })
        .await
        .unwrap();
    }
    c.create_hive(HiveInfo {
        hive_id: "h".into(),
        name: "h".into(),
        description: None,
        members: vec!["seat".into(), "other".into()],
    })
    .await
    .unwrap();
    c.send_as_host(SendMessage {
        message_id: "m".into(),
        sender: "ignored".into(),
        destination: Destination::Hive("h".into()),
        body: "task".into(),
        thread: None,
        only_for: vec![],
        starters: vec!["seat".into(), "other".into()],
    })
    .await
    .unwrap();
    c.run_until_idle().await.unwrap();
    c.release("other").await.unwrap();
    c.run_until_idle().await.unwrap();
    assert_eq!(runner.evidence().unwrap().len(), 2);
    assert_eq!(
        c.episodes().unwrap()[0].phase,
        EpisodePhase::AwaitingRelease
    );
    c.release("seat").await.unwrap();
    c.run_until_idle().await.unwrap();
    assert_eq!(c.episodes().unwrap()[0].phase, EpisodePhase::Settled);
}

fn two_seat_registry() -> VerifierRegistry {
    VerifierRegistry::new(vec![
        VerifierDeclaration {
            node_id: "first".into(),
            owner_seat: "seat".into(),
            verifier: HostVerifier::Golden {
                expected: b"yes".to_vec(),
            },
        },
        VerifierDeclaration {
            node_id: "second".into(),
            owner_seat: "other".into(),
            verifier: HostVerifier::Golden {
                expected: b"yes".to_vec(),
            },
        },
    ])
    .unwrap()
}
