//! Unit tests for seat memory bindings, roots and the `CortexDB` engine.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use tinymemory_api::conformance::reference::ReferenceEngine;

use super::*;

fn engine() -> Arc<dyn MemoryEngine> {
    Arc::new(ReferenceEngine::new())
}

#[test]
fn each_seat_keeps_its_own_namespace_by_default() {
    let memory = SwarmMemory::new(engine(), "team:erdos-779").unwrap();
    assert!(!memory.is_shared());
    assert_eq!(memory.root(), "team:erdos-779");
    for seat in ["lead", "scout"] {
        assert_eq!(memory.agent_id(seat), seat);
        let binding = memory.binding(seat);
        assert_eq!(binding.agent_id(), seat);
        assert_eq!(binding.root_namespace(), Some("team:erdos-779"));
    }
    assert_eq!(memory.namespace("scout"), "team:erdos-779/agent:scout");
}

#[test]
fn shared_memory_binds_every_seat_to_one_namespace() {
    let memory = SwarmMemory::new(engine(), "team:erdos-779")
        .unwrap()
        .shared(true);
    assert!(memory.is_shared());
    for seat in ["lead", "scout", "skeptic"] {
        assert_eq!(memory.agent_id(seat), SHARED_MEMORY_AGENT);
        assert_eq!(memory.binding(seat).agent_id(), SHARED_MEMORY_AGENT);
        assert_eq!(
            memory.binding(seat).root_namespace(),
            Some("team:erdos-779")
        );
        assert_eq!(memory.namespace(seat), "team:erdos-779/agent:hive");
    }
}

#[test]
fn a_root_every_unbound_agent_shares_or_that_is_no_namespace_is_refused() {
    for root in ["", "  ", "root", "not a namespace"] {
        assert!(
            matches!(SwarmMemory::new(engine(), root), Err(Error::Adapter(_))),
            "{root:?} was accepted"
        );
    }
}

#[test]
fn a_cortexdb_engine_needs_a_key_and_never_shows_it() {
    let memory = SwarmMemory::cortexdb("http://127.0.0.1:3141", "sk-cortex-unit", "team:t")
        .unwrap()
        .shared(true);
    let shown = format!("{memory:?}");
    assert!(shown.contains("team:t"), "{shown}");
    assert!(shown.contains("shared: true"), "{shown}");
    assert!(!shown.contains("sk-cortex-unit"), "{shown}");
    assert!(matches!(
        SwarmMemory::cortexdb("http://127.0.0.1:3141", " ", "team:t"),
        Err(Error::MemoryEngine(_))
    ));
}
