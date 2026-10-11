//! Retreat requests, idea extraction and memory across runs.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use tinybackrooms::ScriptedConversant;

fn config() -> manifest::Retreat {
    manifest::Retreat {
        enabled: true,
        variations: 2,
        turns: 2,
        ideas: 3,
        mode: Some("brainstorm".into()),
        template: Some("fugue".into()),
        model: None,
        temperature: Some(1.1),
        memory: true,
    }
}

fn backend() -> Arc<dyn Conversant> {
    Arc::new(ScriptedConversant::new(
        |turn| Ok(format!("{} riffs", turn.seat.role)),
        |call| {
            Ok(json!({"items": [
                {"idea": format!("{} cache recall", call.seat.run_id), "why": "fewer misses"},
                {"idea": format!("{} rerank by date", call.seat.run_id), "why": ""},
            ]}))
        },
    ))
}

#[test]
fn builds_a_request_from_the_settings_and_memory() {
    let dir = tempfile::tempdir().unwrap();
    let memory = dir.path().join("ideas.json");
    store::write_json(&memory, &vec![IdeaRecord { idea: "old".into(), promoted: Some(false) }]).unwrap();
    let retreat = Retreat::with_backend(backend(), config(), Some(memory));
    let request = serde_json::to_value(retreat.request(4, "improve recall").unwrap()).unwrap();
    assert_eq!(request["input"]["brief"], "improve recall");
    assert_eq!(request["input"]["seeds"][0]["kind"], "prior_idea");
    assert!(request["input"]["seeds"][0]["text"].as_str().unwrap().contains("did not improve"));
    let config = &request["config"];
    assert_eq!((config["variations"].as_u64(), config["generations"].as_u64()), (Some(2), Some(2)));
    assert_eq!(config["items_per_variation"], 2);
    assert_eq!(config["mode"], "brainstorm");
    assert_eq!(config["template"], "fugue");
    assert_eq!(config["seed"], 4);
}

#[test]
fn returns_the_best_ideas_and_remembers_their_outcome() {
    let dir = tempfile::tempdir().unwrap();
    let memory = dir.path().join("retreat/ideas.json");
    let mut retreat = Retreat::with_backend(backend(), config(), Some(memory.clone()));
    let ideas = retreat.ideas(1, "improve recall", dir.path()).unwrap();
    assert_eq!(ideas.len(), 3);
    assert!(ideas.iter().any(|i| i.contains("— fewer misses")));
    assert!(dir.path().join("retreat.json").exists());
    retreat.outcome(1, true).unwrap();
    let records: Vec<IdeaRecord> = store::read_json(&memory).unwrap();
    assert_eq!(records.len(), 3);
    assert!(records.iter().all(|r| r.promoted == Some(true)));
}

#[test]
fn without_memory_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut retreat = Retreat::with_backend(backend(), manifest::Retreat { memory: false, mode: None, template: None, temperature: None, ..config() }, None);
    retreat.ideas(1, "x", dir.path()).unwrap();
    retreat.outcome(1, false).unwrap();
    assert!(retreat.pending.is_empty());
}

#[test]
fn a_failing_backend_is_a_proposer_error() {
    let dir = tempfile::tempdir().unwrap();
    let failing = Arc::new(ScriptedConversant::new(
        |_| Err(tinybackrooms::Error::Backend("down".into())),
        |_| Err(tinybackrooms::Error::Backend("down".into())),
    ));
    let mut retreat = Retreat::with_backend(failing, config(), None);
    assert!(matches!(retreat.ideas(1, "x", dir.path()), Err(Error::Proposer(_))));
}
