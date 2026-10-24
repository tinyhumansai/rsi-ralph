//! Wire contract, privacy and strict proposal decoding.
#![expect(clippy::unwrap_used, reason = "assert test setup and outcomes")]
use super::*;
use rsi_ralph_types::{Operation, Patch, PatchClass, Prediction};
use serde::Serialize;
use std::collections::BTreeMap;

/// A dossier shaped like the core crate's: an epoch, references and the
/// per-seat usage map whose keys redaction must cover.
#[derive(Debug, Default, Serialize)]
struct TestDossier {
    epoch: u64,
    transcript_sequences: Vec<u64>,
    seat_usage: BTreeMap<String, serde_json::Value>,
}

impl RetreatDossier for TestDossier {
    fn epoch(&self) -> u64 {
        self.epoch
    }
}

fn dossier() -> TestDossier {
    TestDossier {
        epoch: 7,
        ..TestDossier::default()
    }
}
fn proposal() -> Proposal {
    Proposal {
        title: "Check outputs".into(),
        patch: Patch {
            class: PatchClass::Context,
            operations: vec![Operation::Context {
                path: "context/main.md".into(),
                item: "verify".into(),
                before: None,
                after: Some("Execute checks".into()),
            }],
        },
        mechanism: "Use execution".into(),
        prediction: Prediction {
            metric: "verified_quality".into(),
            minimum_gain: 0.05,
            nodes: vec!["failure".into()],
        },
        transcript_refs: vec![7],
        risk: "more tokens".into(),
        effort: "one delta".into(),
    }
}
#[test]
fn request_uses_tinybackrooms_wire_fields_and_scoped_memory() {
    let request = build_retreat_request("hive-1", &dossier(), &Package::default(), &[]).unwrap();
    assert_eq!(request.as_object().unwrap().len(), 3);
    assert_eq!(request["config"]["mode"], "critic_pair");
    assert_eq!(request["config"]["scoring"]["select_top_k"], 3);
    assert_eq!(
        request["input"]["memory_namespace"],
        "project:rsi-hive-1/agent:retreat"
    );
    assert_eq!(
        request["config"]["scoring"]["scorers"][0]["kind"],
        "comparative"
    );
    assert_eq!(request["config"]["scoring"]["scorers"][1]["kind"], "judge");
    assert_eq!(
        request["config"]["scoring"]["scorers"][2]["kind"],
        "originality"
    );
    assert_eq!(request["output"]["schema"]["additionalProperties"], false);
    assert!(build_retreat_request("", &dossier(), &Package::default(), &[]).is_err());
    assert!(
        build_retreat_request("bad/agent:other", &dossier(), &Package::default(), &[]).is_err()
    );
    assert!(build_retreat_request("bad space", &dossier(), &Package::default(), &[]).is_err());
}
#[test]
fn private_constitution_factory_settings_and_rejected_payloads_never_leave_host() {
    use tinyhivemind_lang::{MemoryIdentity, MemoryKind, MemoryLifecycle, Seat};
    let mut package = Package::default();
    package.constitution.contamination_canaries = vec!["ORACLE_SECRET".into()];
    package.constitution.evaluation = json!({"tasks":"PRIVATE_TASKS"});
    package.constitution.telemetry = json!({"key":"PRIVATE_TOKEN"});
    package.constitution.protected_namespaces = vec!["project:judge".into()];
    package.manifest.context = vec!["context/public.md".into(), "context/contaminated.md".into()];
    package.documents.insert(
        "context/public.md".into(),
        "<!-- item:check -->\nCheck your work".into(),
    );
    package
        .documents
        .insert("context/contaminated.md".into(), "ORACLE_SECRET".into());
    package
        .documents
        .insert("evaluation/private.md".into(), "PRIVATE_TASKS".into());
    package.manifest.seats.push(Seat {
        id: "worker".into(),
        config: json!({"credentials":["FACTORY_SECRET"]}),
        prompt: "seats/worker.md".into(),
        ..Seat::default()
    });
    package
        .documents
        .insert("seats/worker.md".into(), "Public worker prompt".into());
    package.memory.identities.push(MemoryIdentity {
        id: "judge".into(),
        namespace: "project:judge/agent:oracle".into(),
        kind: MemoryKind::Agent,
        lifecycle: MemoryLifecycle::Hive,
        read_only: true,
        fork_parent: None,
    });
    let rejected = ProposalEvent {
        epoch: 0,
        patch: Patch {
            class: PatchClass::Context,
            operations: vec![Operation::Context {
                path: "context/public.md".into(),
                item: "bad".into(),
                before: None,
                after: Some("ORACLE_SECRET PRIVATE_TASKS".into()),
            }],
        },
        rationale: "ORACLE_SECRET FACTORY_SECRET".into(),
        reason: "ORACLE_SECRET".into(),
        transcript_refs: vec![],
    };
    let request = build_retreat_request("hive", &dossier(), &package, &[rejected])
        .unwrap()
        .to_string();
    for secret in [
        "ORACLE_SECRET",
        "PRIVATE_TASKS",
        "PRIVATE_TOKEN",
        "FACTORY_SECRET",
        "project:judge",
    ] {
        assert!(!request.contains(secret), "leaked {secret}");
    }
    assert!(request.contains("Check your work"));
    assert!(!request.contains("\"constitution\""));
}
#[test]
fn strict_decoder_counts_unknown_fields_invalid_numbers_and_invalid_operation_tags() {
    let valid = serde_json::to_value(proposal()).unwrap();
    let mut unknown = valid.clone();
    unknown["extra"] = json!(true);
    let mut invalid_number = valid.clone();
    invalid_number["prediction"]["minimum_gain"] = json!("NaN");
    let mut invalid_operation = valid.clone();
    invalid_operation["patch"]["operations"][0]["operation"] = json!("rewrite");
    let mut invalid_prediction = valid.clone();
    invalid_prediction["prediction"]["minimum_gain"] = json!(0.0);
    let result = decode_retreat_response(&json!({"items":[{"value":valid},{"value":unknown},{"value":invalid_number},{"value":invalid_operation},{"value":invalid_prediction},{}]})).unwrap();
    assert_eq!(result.proposals, vec![proposal()]);
    assert_eq!(result.invalid_items, 5);
    assert!(decode_retreat_response(&json!({"items":{}})).is_err());
    assert!(decode_retreat_response(&json!({})).is_err());
}
#[test]
fn scripted_transport_receives_real_request_and_returns_ranked_typed_values() {
    struct Scripted;
    impl RetreatTransport for Scripted {
        fn generate(&self, request: &Value) -> Result<Value> {
            assert_eq!(
                request["input"]["memory_namespace"],
                "project:rsi-hive/agent:retreat"
            );
            assert_eq!(request["config"]["mode"], "critic_pair");
            Ok(
                json!({"items":[{"value":proposal(),"run_id":"v0","scores":{},"total":0.8}],"warnings":[]}),
            )
        }
    }
    assert_eq!(
        generate_retreat(&Scripted, "hive", &dossier(), &Package::default(), &[]).unwrap(),
        vec![proposal()]
    );
}
#[test]
fn output_schema_has_all_real_language_operation_variants() {
    let schema = proposal_schema();
    let variants = schema["properties"]["patch"]["properties"]["operations"]["items"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(variants.len(), 5);
    assert_eq!(
        variants
            .iter()
            .map(|v| v["properties"]["operation"]["const"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["json", "document", "prompt", "context", "learning"]
    );
}

#[test]
fn host_transport_errors_are_preserved_and_missing_operation_preconditions_count_as_invalid() {
    struct Failed;
    impl RetreatTransport for Failed {
        fn generate(&self, _: &Value) -> Result<Value> {
            Err(Error::HostFailed)
        }
    }
    assert_eq!(
        generate_retreat(&Failed, "hive", &dossier(), &Package::default(), &[]).unwrap_err(),
        Error::HostFailed
    );
    let mut missing = serde_json::to_value(proposal()).unwrap();
    missing["patch"]["operations"][0]
        .as_object_mut()
        .unwrap()
        .remove("before");
    let output = decode_retreat_response(&json!({"items":[{"value":missing}]})).unwrap();
    assert_eq!(output.proposals, Vec::<Proposal>::new());
    assert_eq!(output.invalid_items, 1);
    assert_eq!(
        decode_retreat_response(&json!({"items":[]}))
            .unwrap()
            .invalid_items,
        0
    );
}
#[test]
fn redaction_covers_dossier_map_keys_and_sensitive_hive_identifiers() {
    let mut package = Package::default();
    package
        .constitution
        .contamination_canaries
        .push("SECRET_SEAT".into());
    let mut dossier = dossier();
    dossier
        .seat_usage
        .insert("SECRET_SEAT".into(), json!({"turns": 1}));
    let request = build_retreat_request("hive", &dossier, &package, &[]).unwrap();
    assert!(!request.to_string().contains("SECRET_SEAT"));
    assert!(build_retreat_request("SECRET_SEAT", &dossier, &package, &[]).is_err());
}

#[test]
fn copied_host_acceptance_constraints_and_read_only_identifiers_are_withheld() {
    use tinyhivemind_lang::Goal;
    let mut package = Package::default();
    package.constitution.hard_constraints = vec!["PRIVATE_CONSTRAINT".into()];
    package.constitution.read_only_memories = vec!["PRIVATE_MEMORY_ID".into()];
    package.manifest.goals.push(Goal {
        id: "goal".into(),
        description: "PRIVATE_ACCEPTANCE PRIVATE_CONSTRAINT".into(),
        acceptance: vec!["PRIVATE_ACCEPTANCE".into()],
        subgoals: vec![],
    });
    package.manifest.context.push("context/main.md".into());
    package.documents.insert(
        "context/main.md".into(),
        "PRIVATE_ACCEPTANCE PRIVATE_CONSTRAINT PRIVATE_MEMORY_ID".into(),
    );
    let request = build_retreat_request("hive", &dossier(), &package, &[])
        .unwrap()
        .to_string();
    for secret in [
        "PRIVATE_ACCEPTANCE",
        "PRIVATE_CONSTRAINT",
        "PRIVATE_MEMORY_ID",
    ] {
        assert!(!request.contains(secret), "leaked {secret}");
    }
}
#[test]
fn side_document_frontmatter_is_withheld_and_its_values_cannot_be_copied_into_public_text() {
    use tinyhivemind_lang::Seat;
    let mut package = Package::default();
    package.manifest.seats.push(Seat {
        id: "worker".into(),
        prompt: "seats/worker.md".into(),
        ..Seat::default()
    });
    package.documents.insert(
        "seats/worker.md".into(),
        "---\n{\"id\":\"worker\",\"config\":{\"token\":\"FRONTMATTER_SECRET\"},\"host_private_setting\":\"UNKNOWN_HEADER_SECRET\"}\n---\nPublic worker instructions"
            .into(),
    );
    package.manifest.context.push("context/main.md".into());
    package.documents.insert(
        "context/main.md".into(),
        "FRONTMATTER_SECRET UNKNOWN_HEADER_SECRET".into(),
    );
    let request = build_retreat_request("hive", &dossier(), &package, &[])
        .unwrap()
        .to_string();
    assert!(!request.contains("FRONTMATTER_SECRET"));
    assert!(!request.contains("UNKNOWN_HEADER_SECRET"));
    assert!(request.contains("Public worker instructions"));
}

#[test]
fn malformed_frontmatter_cannot_escape_the_privacy_projection() {
    let mut package = Package::default();
    package.documents.insert(
        "seats/worker.md".into(),
        "---\nconfig: private\n---\npublic".into(),
    );
    assert_eq!(
        build_retreat_request("hive", &dossier(), &package, &[]).unwrap_err(),
        Error::InvalidProposal
    );
    package
        .documents
        .insert("seats/worker.md".into(), "---\n{}".into());
    assert!(build_retreat_request("hive", &dossier(), &package, &[]).is_err());
}

#[test]
fn actual_tinybackrooms_request_deserializes_and_validates() {
    let typed = typed_retreat_request("hive", &dossier(), &Package::default(), &[]).unwrap();
    typed.validate().unwrap();
    assert_eq!(typed.config.mode, tinybackrooms::Mode::CriticPair);
    assert_eq!(
        typed.input.seeds[0].kind,
        tinybackrooms::SeedKind::Reference
    );
    assert_eq!(
        typed.input.seeds[2].kind,
        tinybackrooms::SeedKind::PriorIdea
    );
    assert_eq!(typed.config.scoring.select_top_k, Some(3));
}
#[tokio::test]
async fn actual_engine_runs_critic_formatter_and_scorers_without_network() {
    use std::sync::{Arc, Mutex};
    use tinybackrooms::{Engine, ScriptedConversant};
    let tools = Arc::new(Mutex::new(Vec::new()));
    let seen = tools.clone();
    let backend = ScriptedConversant::new(
        |request| {
            assert!(
                request
                    .system
                    .starts_with(if request.seat.role == "agent_a" {
                        "You propose."
                    } else {
                        "You attack."
                    })
            );
            Ok(format!(
                "{} proposes or critiques execution",
                request.seat.role
            ))
        },
        move |request| {
            seen.lock().unwrap().push(request.tool.clone());
            match request.tool.as_str() {
                "submit_items" => Ok(json!({"items":[proposal()]})),
                "rate_item" => Ok(json!({"novelty":5,"usefulness":5,"boden":"exploratory"})),
                "rank_items" => Ok(json!({"assessment":"Grounded execution","ranking":[1,2,3]})),
                _ => Err(tinybackrooms::Error::Conversant(
                    "unexpected test tool".into(),
                )),
            }
        },
    );
    let engine = Engine::new(Arc::new(backend));
    let result =
        generate_retreat_with_engine(&engine, "hive", &dossier(), &Package::default(), &[])
            .await
            .unwrap();
    assert_eq!(result.invalid_items, 0);
    assert_eq!(result.proposals.len(), 3);
    assert!(result.proposals.iter().all(|value| value == &proposal()));
    let tools = tools.lock().unwrap();
    for tool in ["submit_items", "rate_item", "rank_items"] {
        assert!(tools.iter().any(|seen| seen == tool));
    }
}
#[tokio::test]
async fn actual_engine_failure_becomes_host_failure() {
    use std::sync::Arc;
    let engine = tinybackrooms::Engine::new(Arc::new(tinybackrooms::ScriptedConversant::new(
        |_| Err(tinybackrooms::Error::Conversant("backend offline".into())),
        |_| Err(tinybackrooms::Error::Conversant("backend offline".into())),
    )));
    assert_eq!(
        generate_retreat_with_engine(&engine, "hive", &dossier(), &Package::default(), &[])
            .await
            .unwrap_err(),
        Error::HostFailed
    );
}

/// Answers with the given items, or fails.
#[derive(Debug)]
struct Canned(Option<Value>);

impl RetreatTransport for Canned {
    fn generate(&self, _: &Value) -> Result<Value> {
        self.0.clone().ok_or(Error::HostFailed)
    }
}

fn brief(hive_id: &str) -> RetreatBrief<TestDossier> {
    RetreatBrief {
        hive_id: hive_id.into(),
        dossier: dossier(),
        incumbent: Package::default(),
        rejected: vec![],
    }
}

#[test]
fn the_retreat_evolver_keeps_rank_order_and_reports_unusable_items_after_it() {
    use crate::{EvolveContext, Evolver};
    let mut second = proposal();
    second.title = "Second".into();
    let evolver = RetreatEvolver::new(Canned(Some(json!({"items": [
        {"value": proposal()},
        {"value": {"title": "broken"}},
        {"value": second},
    ]}))));
    let context = EvolveContext {
        epoch: 7,
        index: 0,
        seed: 7,
    };
    let proposed = evolver
        .propose(&brief("hive"), &rsi_ralph_types::Goal::new("g"), &context)
        .unwrap();
    assert_eq!(proposed.tokens, 0);
    let titles: Vec<_> = proposed
        .proposals
        .iter()
        .map(|p| {
            p.as_ref()
                .map(|p| p.title.clone())
                .map_err(ToString::to_string)
        })
        .collect();
    assert_eq!(
        titles,
        vec![
            Ok("Check outputs".to_owned()),
            Ok("Second".to_owned()),
            Err(Error::InvalidProposal.to_string())
        ]
    );
    assert!(format!("{evolver:?}").contains("RetreatEvolver"));
}

#[test]
fn the_retreat_evolver_refuses_unsafe_input_and_passes_transport_failures() {
    use crate::{EvolveContext, Evolver};
    let context = EvolveContext {
        epoch: 7,
        index: 0,
        seed: 7,
    };
    let goal = rsi_ralph_types::Goal::new("g");
    let ok = RetreatEvolver::new(Canned(Some(json!({"items": []}))));
    assert_eq!(
        ok.propose(&brief("bad space"), &goal, &context),
        Err(Error::InvalidProposal)
    );
    let down = RetreatEvolver::new(Canned(None));
    assert_eq!(
        down.propose(&brief("hive"), &goal, &context),
        Err(Error::HostFailed)
    );
    let empty = RetreatEvolver::new(Canned(Some(json!({}))));
    assert_eq!(
        empty.propose(&brief("hive"), &goal, &context),
        Err(Error::InvalidProposal)
    );
}

#[test]
fn the_retreat_evolver_rejects_a_response_with_no_items() {
    use crate::{EvolveContext, Evolver};
    let context = EvolveContext {
        epoch: 7,
        index: 0,
        seed: 7,
    };
    let none = RetreatEvolver::new(Canned(Some(json!({"items": []}))));
    assert_eq!(
        none.propose(&brief("hive"), &rsi_ralph_types::Goal::new("g"), &context),
        Err(Error::InvalidProposal)
    );
}

#[test]
fn the_engine_seed_is_the_dossier_epoch() {
    let request = build_retreat_request("hive", &dossier(), &Package::default(), &[]).unwrap();
    assert_eq!(request["config"]["seed"], 7);
}

#[test]
fn prior_idea_seeds_carry_a_coarse_outcome_and_no_private_fields() {
    let event = |reason: &str| ProposalEvent {
        epoch: 4,
        patch: Patch {
            class: PatchClass::Context,
            operations: vec![Operation::Context {
                path: "context/public.md".into(),
                item: "PRIVATE_ITEM".into(),
                before: None,
                after: Some("PRIVATE_BODY".into()),
            }],
        },
        rationale: "PRIVATE_RATIONALE".into(),
        reason: reason.into(),
        transcript_refs: vec![9],
    };
    let events = [
        "Harmful",
        "NoGain",
        "Cooldown",
        "ApprovalDenied",
        "Guard",
        "NotReplicated",
    ]
    .map(event);
    let mut priors: Vec<PriorIdea> = events.iter().map(PriorIdea::from_event).collect();
    priors[5] = priors[5].clone().with_outcome(PriorOutcome::Regressed);
    assert_eq!(priors[0].outcome(), Some(PriorOutcome::Crashed));
    let request =
        build_retreat_request_with_priors("hive", &dossier(), &Package::default(), &priors)
            .unwrap();
    let seeds: serde_json::Value =
        serde_json::from_str(request["input"]["seeds"][2]["text"].as_str().unwrap()).unwrap();
    let seed = |reason: &str, outcome: serde_json::Value| {
        json!({"epoch": 4, "class": "context", "reason": reason, "operation_count": 1,
            "outcome": outcome})
    };
    assert_eq!(
        seeds,
        json!([
            seed("Harmful", json!("crashed")),
            seed("NoGain", json!("flat")),
            seed("Cooldown", json!("cooled")),
            seed("ApprovalDenied", json!("denied")),
            seed("Guard", json!(null)),
            seed("NotReplicated", json!("regressed")),
        ])
    );
    let text = request.to_string();
    for private in [
        "PRIVATE_ITEM",
        "PRIVATE_BODY",
        "PRIVATE_RATIONALE",
        "delta",
        "upper",
    ] {
        assert!(!text.contains(private), "leaked {private}");
    }
    // The original builder derives the same seeds from reasons alone.
    let plain = build_retreat_request("hive", &dossier(), &Package::default(), &events).unwrap();
    let plain: serde_json::Value =
        serde_json::from_str(plain["input"]["seeds"][2]["text"].as_str().unwrap()).unwrap();
    assert_eq!(plain[5], seed("NotReplicated", json!("flat")));
    assert_eq!(
        typed_retreat_request_with_priors("hive", &dossier(), &Package::default(), &priors)
            .unwrap()
            .input
            .seeds
            .len(),
        3
    );
}
