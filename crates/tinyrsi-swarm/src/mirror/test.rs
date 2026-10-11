//! Unit tests for mirroring the coordinator transcript.
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, mpsc};

use tinyhivemind_hives::{
    AgentRegistration, AgentRunner, CoordinatorOptions, HiveInfo, MemoryStorage, SendMessage,
    TurnFuture, TurnRequest,
};

use super::*;
use crate::event::Redactor;

/// A runner the tests never schedule: they only enqueue and read.
struct Idle;

impl AgentRunner for Idle {
    fn run(&self, _: TurnRequest) -> TurnFuture {
        Box::pin(std::future::pending())
    }
}

async fn coordinator() -> Coordinator {
    let coordinator = Coordinator::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        CoordinatorOptions::default(),
    )
    .await
    .unwrap();
    for agent in ["lead", "impl"] {
        coordinator
            .register_agent(AgentRegistration {
                agent_id: agent.into(),
                runtime_id: "runtime".into(),
                runner: Arc::new(Idle),
            })
            .await
            .unwrap();
    }
    coordinator
        .create_hive(HiveInfo {
            hive_id: "lab".into(),
            name: "Lab".into(),
            description: None,
            members: Vec::new(),
        })
        .await
        .unwrap();
    coordinator.join_hive("lab", "lead").await.unwrap();
    coordinator
}

fn send(id: &str, destination: Destination, body: &str) -> SendMessage {
    SendMessage {
        message_id: id.into(),
        sender: String::new(),
        destination,
        body: body.into(),
        thread: None,
        only_for: Vec::new(),
        starters: Vec::new(),
    }
}

#[tokio::test]
async fn each_message_is_mirrored_exactly_once() {
    let coordinator = coordinator().await;
    let dir = tempfile::tempdir().unwrap();
    let (sender, receiver) = mpsc::channel();
    let trace = Trace::create(
        &dir.path().join("events.jsonl"),
        Redactor::default(),
        sender,
    )
    .unwrap();
    let mirror = Mirror::default();

    mirror.flush(&coordinator, &trace).unwrap();
    assert_eq!(receiver.try_iter().count(), 0);

    coordinator
        .send_as_host(send(
            "brief",
            Destination::Agent("lead".into()),
            "the brief",
        ))
        .await
        .unwrap();
    mirror.flush(&coordinator, &trace).unwrap();
    coordinator
        .send_as_host(send(
            "post",
            Destination::Hive("lab".into()),
            &"x".repeat(5000),
        ))
        .await
        .unwrap();
    mirror.flush(&coordinator, &trace).unwrap();
    mirror.flush(&coordinator, &trace).unwrap();

    let events: Vec<SwarmEvent> = receiver.try_iter().collect();
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(
        matches!(&events[0], SwarmEvent::AgentMessage { to, body, .. } if to == "lead" && body == "the brief")
    );
    assert!(
        matches!(&events[1], SwarmEvent::AgentMessage { to, body, .. }
        if to == "hive:lab" && body.chars().count() == BODY_CHARS + 1)
    );
    let sequences: Vec<u64> = events
        .iter()
        .filter_map(|event| match event {
            SwarmEvent::AgentMessage { sequence, .. } => Some(*sequence),
            _ => None,
        })
        .collect();
    assert!(sequences[0] < sequences[1]);
}
