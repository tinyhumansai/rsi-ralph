//! The supervisor through the evolution port, and proposal validation.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::supervisor::{TOOL, schema};
use super::*;
use crate::{EvolveContext, Evolver};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tinyrsi_model::{ModelFuture, ModelPort, ModelReply, ModelRequest};
use tinyrsi_types::Goal;

/// Replays canned tool arguments, then fails; records prompts and seeds.
struct Scripted {
    replies: Mutex<VecDeque<String>>,
    seen: Mutex<Vec<(String, u64)>>,
    temperatures: Mutex<Vec<f64>>,
}

impl Scripted {
    fn new(replies: Vec<String>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            seen: Mutex::new(vec![]),
            temperatures: Mutex::new(vec![]),
        })
    }
}

impl ModelPort for Scripted {
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        self.seen
            .lock()
            .unwrap()
            .push((request.prompt().to_owned(), request.seed()));
        self.temperatures
            .lock()
            .unwrap()
            .push(request.temperature().unwrap_or(0.0));
        let next = self.replies.lock().unwrap().pop_front();
        Box::pin(async move {
            let content = next.ok_or(tinyrsi_model::Error::CallFailed)?.into_bytes();
            Ok(ModelReply {
                content,
                input_tokens: 100,
                output_tokens: 50,
            })
        })
    }
}

fn reply(path: &str) -> String {
    serde_json::json!({
        "rationale": "lower the loss",
        "hypothesis": "loss drops",
        "files": [{"path": path, "content": "echo METRIC loss=0.1"}]
    })
    .to_string()
}

fn brief() -> ScriptBrief {
    ScriptBrief {
        prompt: "the rendered prompt".into(),
        editable: vec!["run.sh".into()],
        temperature: None,
    }
}

fn context(seed: u64) -> EvolveContext {
    EvolveContext {
        epoch: 1,
        index: 0,
        seed,
    }
}

fn proposal(paths: &[&str]) -> ScriptProposal {
    ScriptProposal {
        approach: String::new(),
        differs_from_previous: None,
        rationale: "r".into(),
        hypothesis: "h".into(),
        files: paths
            .iter()
            .map(|p| ProposedFile {
                path: (*p).into(),
                content: "x".into(),
            })
            .collect(),
    }
}

#[test]
fn rejects_proposal_paths_outside_the_editable_set() {
    let editable = vec![PathBuf::from("run.sh"), PathBuf::from("lib/model.py")];
    assert!(
        proposal(&["run.sh", "lib/model.py"])
            .validate(&editable)
            .is_ok()
    );
    for bad in [
        "/etc/passwd",
        "../run.sh",
        "lib/../run.sh",
        "./run.sh",
        "other.sh",
        "",
    ] {
        assert_eq!(
            proposal(&[bad]).validate(&editable).unwrap_err(),
            Error::ProposalPathOutsideEditable,
            "{bad}"
        );
    }
}

#[test]
fn rejects_empty_or_repeated_proposals_as_malformed() {
    let editable = vec![PathBuf::from("run.sh")];
    assert_eq!(
        proposal(&[]).validate(&editable).unwrap_err(),
        Error::SupervisorMalformed
    );
    assert_eq!(
        proposal(&["run.sh", "run.sh"])
            .validate(&editable)
            .unwrap_err(),
        Error::SupervisorMalformed
    );
    let mut blank = proposal(&["run.sh"]);
    blank.rationale = " ".into();
    assert_eq!(
        blank.validate(&editable).unwrap_err(),
        Error::SupervisorMalformed
    );
}

#[test]
fn supervisor_rejects_unusable_limits() {
    let port = Scripted::new(vec![]);
    for limits in [
        SupervisorLimits {
            call_tokens: 0,
            ..SupervisorLimits::default()
        },
        SupervisorLimits {
            output_tokens: 0,
            ..SupervisorLimits::default()
        },
        SupervisorLimits {
            output_tokens: 300_000,
            ..SupervisorLimits::default()
        },
        SupervisorLimits {
            timeout_ms: 0,
            ..SupervisorLimits::default()
        },
        SupervisorLimits {
            timeout_ms: 300_001,
            ..SupervisorLimits::default()
        },
    ] {
        assert_eq!(
            ScriptSupervisor::new(port.clone(), limits).unwrap_err(),
            Error::InvalidScriptSubject
        );
    }
    let supervisor = ScriptSupervisor::new(port, SupervisorLimits::default()).unwrap();
    assert!(format!("{supervisor:?}").contains("limits"));
}

#[test]
fn proposes_one_validated_proposal_per_call_with_the_context_seed() {
    let port = Scripted::new(vec![
        reply("run.sh"),
        "not json".into(),
        r#"{"rationale":"r","hypothesis":"h","files":[],"extra":1}"#.into(),
        reply("../escape.sh"),
    ]);
    let supervisor = ScriptSupervisor::new(port.clone(), SupervisorLimits::default()).unwrap();
    let goal = Goal::new("minimize the loss");
    let first = supervisor.propose(&brief(), &goal, &context(9)).unwrap();
    assert_eq!(first.tokens, 150);
    assert_eq!(first.proposals.len(), 1);
    assert_eq!(first.proposals[0].as_ref().unwrap().files[0].path, "run.sh");
    for expected in [
        Error::SupervisorMalformed,
        Error::SupervisorMalformed,
        Error::ProposalPathOutsideEditable,
    ] {
        let proposed = supervisor.propose(&brief(), &goal, &context(9)).unwrap();
        assert_eq!(proposed.proposals, vec![Err(expected)]);
    }
    assert_eq!(
        supervisor
            .propose(&brief(), &goal, &context(9))
            .unwrap_err(),
        Error::Model(tinyrsi_model::Error::CallFailed)
    );
    let seen = port.seen.lock().unwrap();
    assert_eq!(seen[0], ("the rendered prompt".to_owned(), 9));
}

#[test]
fn the_tool_schema_limits_paths_to_the_editable_set() {
    assert_eq!(TOOL, "propose_script");
    let schema = schema(&[PathBuf::from("run.sh"), PathBuf::from("lib/m.py")]);
    let paths = &schema["properties"]["files"]["items"]["properties"]["path"]["enum"];
    assert_eq!(paths, &serde_json::json!(["run.sh", "lib/m.py"]));
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["required"],
        serde_json::json!(["approach", "rationale", "hypothesis", "files"])
    );
    assert_eq!(schema["properties"]["approach"]["maxLength"], 60);
    assert_eq!(
        schema["properties"]["differs_from_previous"]["type"],
        serde_json::json!(["string", "null"])
    );
}

#[test]
fn pins_the_proposal_wire_shape_with_and_without_an_approach() {
    let mut labelled = proposal(&["run.sh"]);
    labelled.approach = "tabu search".into();
    labelled.differs_from_previous = Some("longer tenure".into());
    let text = r#"{"approach":"tabu search","differs_from_previous":"longer tenure","rationale":"r","hypothesis":"h","files":[{"path":"run.sh","content":"x"}]}"#;
    assert_eq!(serde_json::to_string(&labelled).unwrap(), text);
    assert_eq!(
        serde_json::from_str::<ScriptProposal>(text).unwrap(),
        labelled
    );
    // A reply written before approaches existed still decodes, unlabelled.
    let legacy = r#"{"rationale":"r","hypothesis":"h","files":[{"path":"run.sh","content":"x"}]}"#;
    let decoded = serde_json::from_str::<ScriptProposal>(legacy).unwrap();
    assert_eq!(decoded, proposal(&["run.sh"]));
    assert_eq!(decoded.approach_label(), None);
    let null = r#"{"approach":"x","differs_from_previous":null,"rationale":"r","hypothesis":"h","files":[]}"#;
    assert_eq!(
        serde_json::from_str::<ScriptProposal>(null)
            .unwrap()
            .differs_from_previous,
        None
    );
}

#[test]
fn approach_labels_are_collapsed_and_bounded() {
    let mut p = proposal(&["run.sh"]);
    for blank in ["", "  ", "\n\t"] {
        p.approach = blank.into();
        assert_eq!(p.approach_label(), None, "{blank:?}");
    }
    p.approach = "  Tabu \n  search ".into();
    assert_eq!(p.approach_label().as_deref(), Some("Tabu search"));
    p.approach = "é".repeat(APPROACH_CHARS);
    assert_eq!(p.approach_label(), Some("é".repeat(APPROACH_CHARS)));
    p.approach = "é".repeat(APPROACH_CHARS + 5);
    let cut = p.approach_label().unwrap();
    assert_eq!(cut.chars().count(), APPROACH_CHARS);
    assert!(cut.ends_with('…'));
}

#[test]
fn the_brief_temperature_reaches_the_model_request() {
    let port = Scripted::new(vec![reply("run.sh"), reply("run.sh")]);
    let supervisor = ScriptSupervisor::new(port.clone(), SupervisorLimits::default()).unwrap();
    let goal = Goal::new("minimize the loss");
    supervisor.propose(&brief(), &goal, &context(1)).unwrap();
    let hot = ScriptBrief {
        temperature: Some(0.8),
        ..brief()
    };
    supervisor.propose(&hot, &goal, &context(1)).unwrap();
    assert_eq!(*port.temperatures.lock().unwrap(), [0.0, 0.8]);
    let invalid = ScriptBrief {
        temperature: Some(3.0),
        ..brief()
    };
    assert_eq!(
        supervisor
            .propose(&invalid, &goal, &context(1))
            .unwrap_err(),
        Error::Model(tinyrsi_model::Error::InvalidRequest)
    );
}

#[test]
fn plain_relative_paths_have_only_normal_components() {
    assert!(is_plain_relative(Path::new("a/b.rs")));
    // `Path::components` drops an interior `.`, so only a leading one counts.
    for bad in ["", "/a", "../a", "./a", "a/../b"] {
        assert!(!is_plain_relative(Path::new(bad)), "{bad}");
    }
}
