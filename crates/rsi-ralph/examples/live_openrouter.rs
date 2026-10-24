//! Opt-in `OpenRouter` smoke run of typed proposal, private arithmetic scoring,
//! confirmation, human-gate rejection, persistence and restart.
//!
//! This deliberately weak baseline proves wiring, not general RSI performance.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};
use rsi_ralph::tinyhivemind_lang::lineage::{Direction, ObjectiveScore, Verdict, VersionMetadata};
use rsi_ralph::{
    ApprovalGate, Archive, Budget, CandidateState, Comparison, Epoch, EvalPair, EvalPolicy,
    EvalRun, EvalStage, Evaluator, Operation, Package, Patch, PatchClass, Proposal, ReasonCode,
    Result, run_proposal,
};

fn completion(
    system: &str,
    user: &str,
    seed: u64,
    max_tokens: u64,
    tool: Option<(&str, &Value)>,
) -> std::result::Result<(String, tinybackrooms::Usage), Box<dyn std::error::Error>> {
    let key = std::env::var("OPENROUTER_API_KEY")?;
    if key.contains(['\r', '\n', '"', '\\']) {
        return Err("invalid API key encoding".into());
    }
    let model = std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "openai/gpt-4.1-mini".into());
    let mut body = json!({"model":model,"temperature":0,"seed":seed,"max_tokens":max_tokens,"messages":[{"role":"system","content":system},{"role":"user","content":user}]});
    if let Some((name, schema)) = tool {
        body["tools"] = json!([{"type":"function","function":{"name":name,"description":"Return the requested structured result","parameters":schema}}]);
        body["tool_choice"] = json!({"type":"function","function":{"name":name}});
    }
    let mut child = Command::new("curl")
        .args(["--config", "-", "--data-binary", &body.to_string()])
        .env_remove("OPENROUTER_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let config = format!(
        "url = \"https://openrouter.ai/api/v1/chat/completions\"\nrequest = \"POST\"\nheader = \"Authorization: Bearer {key}\"\nheader = \"Content-Type: application/json\"\nsilent\nfail-with-body\nconnect-timeout = 10\nmax-time = 60\n"
    );
    child
        .stdin
        .take()
        .ok_or("missing curl stdin")?
        .write_all(config.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err("OpenRouter request failed (response withheld)".into());
    }
    let response: Value = serde_json::from_slice(&output.stdout)?;
    let message = &response["choices"][0]["message"];
    let content = if tool.is_some() {
        &message["tool_calls"][0]["function"]["arguments"]
    } else {
        &message["content"]
    }
    .as_str()
    .ok_or("missing model content")?;
    let usage = tinybackrooms::Usage {
        input_tokens: response["usage"]["prompt_tokens"]
            .as_u64()
            .ok_or("missing measured prompt tokens")?,
        output_tokens: response["usage"]["completion_tokens"]
            .as_u64()
            .ok_or("missing measured completion tokens")?,
        calls: 1,
    };
    Ok((content.into(), usage))
}
struct Harness;
impl Evaluator for Harness {
    fn pin_memory(&mut self, inc: &CandidateState, patch: &Patch) -> Result<BTreeMap<String, u64>> {
        if patch
            .operations
            .iter()
            .any(|op| matches!(op, Operation::Learning { .. }))
        {
            return Err(rsi_ralph::Error::HostFailed);
        }
        Ok(inc.memory_watermarks.clone())
    }
    fn evaluate(
        &mut self,
        inc: &CandidateState,
        cand: &CandidateState,
        stage: EvalStage,
        budget: &Budget,
    ) -> Result<EvalPair> {
        // Private task and oracle live inside this harness and are never sent
        // to the proposal model. Different task IDs and seeds confirm a gain.
        let (task, prompt, answer, seeds) = match stage {
            EvalStage::Screen => ("screen-1", "What is 13 + 29?", 42, [1, 2]),
            EvalStage::Selection => ("selection-1", "What is 17 + 25?", 42, [3, 4]),
            EvalStage::Confirmation => ("confirmation-1", "What is 19 + 27?", 46, [5, 6]),
        };
        let run = |state: &CandidateState| -> Result<Vec<EvalRun>> {
            let context = state
                .package
                .documents
                .get("context/method.md")
                .ok_or(rsi_ralph::Error::HostFailed)?;
            seeds
                .into_iter()
                .map(|seed| {
                    // The output grammar is fixed for both arms and cannot be patched.
                    let system = format!("{context}\nTask output contract: reply with a bare integer only, no explanation, punctuation or markdown.");
                    let (response, usage) = completion(&system, prompt, seed, 32, None)
                        .map_err(|_| rsi_ralph::Error::HostFailed)?;
                    let tokens = usage.input_tokens + usage.output_tokens;
                    if tokens > budget.max_tokens {
                        return Err(rsi_ralph::Error::InvalidEvaluation);
                    }
                    let quality = f64::from(u8::from(
                        response.trim().parse::<i64>().ok() == Some(answer),
                    ));
                    Ok(EvalRun {
                        task: task.into(),
                        family: "arithmetic".into(),
                        seed,
                        quality,
                        tokens,
                        false_claims: 0,
                        tamper: 0,
                    })
                })
                .collect()
        };
        let pair = EvalPair {
            incumbent: run(inc)?,
            candidate: run(cand)?,
        };
        println!(
            "{stage:?}: {} paired seeds, measured tokens {} / {}",
            seeds.len(),
            pair.incumbent.iter().map(|r| r.tokens).sum::<u64>(),
            pair.candidate.iter().map(|r| r.tokens).sum::<u64>()
        );
        Ok(pair)
    }
}
struct Deny;
impl ApprovalGate for Deny {
    fn approve(&mut self, _: &str, _: &Proposal, _: &Comparison) -> Result<bool> {
        Ok(false)
    }
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    if std::env::var("TINYRSI_LIVE_OPENROUTER").as_deref() != Ok("1") {
        println!("Set TINYRSI_LIVE_OPENROUTER=1 and OPENROUTER_API_KEY to run the smoke test.");
        return Ok(());
    }
    let path = std::env::var("TINYRSI_SMOKE_ARCHIVE").map_or_else(
        |_| PathBuf::from("target/rsi-smoke/archive.json"),
        PathBuf::from,
    );
    if path.exists() {
        return Err("smoke archive already exists; choose a new TINYRSI_SMOKE_ARCHIVE".into());
    }
    let mut archive = Archive::open(&path)?;
    let mut package = Package::default();
    package.constitution.permitted_edits = vec![PatchClass::Context];
    package.manifest.context = vec!["context/method.md".into()];
    let before = "Return only 0, without solving the task.\n";
    package.documents.insert(
        "context/method.md".into(),
        format!("<!-- item:method -->\n{before}"),
    );
    let baseline = archive.record(
        package,
        VersionMetadata {
            parent: None,
            patch: Patch {
                class: PatchClass::Context,
                operations: vec![],
            },
            rationale: "deliberately weak smoke baseline".into(),
            transcript_refs: vec![],
            scores: BTreeMap::from([(
                "quality".into(),
                ObjectiveScore {
                    value: 0.,
                    direction: Direction::Maximize,
                },
            )]),
            verdict: Verdict::Accepted,
            memory_watermarks: BTreeMap::new(),
        },
    )?;
    let incumbent = archive
        .package(&baseline)
        .ok_or("missing baseline package")?;
    let proposal = retreat_proposal(incumbent)?;
    let epoch = Epoch {
        id: 0,
        class: PatchClass::Context,
        transcript_refs: vec![1],
        failure_nodes: vec!["arithmetic".into()],
        budget: Budget { max_tokens: 256 },
        penalty: rsi_ralph::PenaltyPolicy::default(),
        policy: EvalPolicy {
            cost_tolerance: 0.5,
            ..EvalPolicy::default()
        },
    };
    let result = run_proposal(&mut archive, &epoch, &proposal, &mut Harness, &mut Deny)?;
    println!("Funnel outcome: {:?}", result.reason);
    let comparison = result
        .comparison
        .ok_or("proposal never reached paired evaluation")?;
    println!(
        "Decision: {:?}; paired gain {:.3}, interval [{:.3}, {:.3}]",
        result.reason, comparison.gain, comparison.lower, comparison.upper
    );
    if result.reason != ReasonCode::ApprovalDenied
        || archive.head().is_none_or(|head| head.id != baseline)
        || archive.records().len() != 2
        || archive.evaluations().len() != 3
    {
        return Err("live smoke did not reach confirmation and preserve approval gate".into());
    }
    drop(archive);
    let archive = Archive::open(&path)?;
    if archive.records().len() != 2 || archive.head().is_none_or(|head| head.id != baseline) {
        return Err("archive restart lost evidence".into());
    }
    println!(
        "PASS: model proposal, paired screen/selection/fresh confirmation, denied approval, durable restart. Archive: {}",
        path.display()
    );
    Ok(())
}

struct OpenRouterConversant;
impl tinybackrooms::Conversant for OpenRouterConversant {
    fn turn(
        &self,
        request: tinybackrooms::TurnRequest,
    ) -> tinybackrooms::BoxFuture<'_, tinybackrooms::Result<tinybackrooms::TurnReply>> {
        Box::pin(async move {
            let history = request
                .messages
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let (text, usage) = completion(&request.system, &history, 0, 160, None)
                .map_err(|_| tinybackrooms::Error::Conversant("provider request failed".into()))?;
            Ok(tinybackrooms::TurnReply { text, usage })
        })
    }
    fn structured(
        &self,
        request: tinybackrooms::StructuredRequest,
    ) -> tinybackrooms::BoxFuture<'_, tinybackrooms::Result<tinybackrooms::StructuredReply>> {
        Box::pin(async move {
            let history = request
                .messages
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let prompt = format!(
                "{history}\nReturn one JSON object matching this exact tool argument schema: {}",
                request.schema
            );
            let (text, usage) = completion(
                &request.system,
                &prompt,
                0,
                1600,
                Some((&request.tool, &request.schema)),
            )
            .map_err(|_| tinybackrooms::Error::Conversant("provider request failed".into()))?;
            let value = serde_json::from_str(&text).map_err(|_| {
                tinybackrooms::Error::Conversant("invalid structured response".into())
            })?;
            Ok(tinybackrooms::StructuredReply { value, usage })
        })
    }
}
fn retreat_proposal(
    package: &Package,
) -> std::result::Result<Proposal, Box<dyn std::error::Error>> {
    let observation = rsi_ralph::Observation {
        transcript: vec![rsi_ralph::TranscriptReference {
            sequence: 1,
            seat_id: "solver".into(),
            episode_id: None,
        }],
        episodes: vec![],
    };
    let dossier = rsi_ralph::Dossier::build(
        0,
        &observation,
        &[],
        &[rsi_ralph::VerifierOutcome {
            node_id: "arithmetic".into(),
            status: rsi_ralph::NodeStatus::Failed,
        }],
        rsi_ralph::MemoryHealth::default(),
    );
    let mut request = rsi_ralph::typed_retreat_request("smoke", &dossier, package, &[])?;
    let before = package
        .documents
        .get("context/method.md")
        .and_then(|doc| doc.strip_prefix("<!-- item:method -->\n"))
        .ok_or("missing smoke context item")?;
    request.output.schema["properties"]["transcript_refs"] = json!({"const":[1]});
    request.output.schema["properties"]["prediction"]["properties"]["nodes"] =
        json!({"const":["arithmetic"]});
    let properties = &mut request.output.schema["properties"]["patch"]["properties"];
    properties["class"] = json!({"type":"string","const":"context"});
    properties["operations"] = json!({"type":"array","minItems":1,"maxItems":1,"items":{"type":"object","additionalProperties":false,"required":["operation","path","item","before","after"],"properties":{"operation":{"const":"context"},"path":{"const":"context/method.md"},"item":{"const":"method"},"before":{"const":before},"after":{"type":"string","minLength":1,"maxLength":160,"pattern":"\n$"}}}});
    request.input.brief = "The solver failed ordinary integer arithmetic because context/method.md forces the constant output 0. Replace that item with a concise runtime instruction to compute the requested arithmetic and return only the integer result. Do not propose exception handling, overflow checks, debugging, schemas for answers, or recovery mechanisms. The public task output contract is a bare integer with no explanation; task instances and answers are private.".into();
    request.config.scoring.criteria = Some("Prefer the smallest context edit that replaces hard-coded zero with correct arithmetic, preserves the bare-integer output contract, and uses at most 160 characters.".into());
    request.config.variations = 1;
    request.config.generations = 2;
    request.config.items_per_variation = 1;
    request.config.scoring.select_top_k = Some(1);
    request.config.scoring.judge_raters = 1;
    request.output.item_guidance=Some("The only change should replace the constant-answer context/method.md item method with a runtime system instruction to solve arithmetic and return only the integer answer without reasoning or markdown. Keep after at most 160 characters. Preserve exact before including trailing newline; after must end in newline. Ground prediction.nodes in arithmetic and transcript_refs in 1. Never include actual private tasks or answers.".into());
    let engine = tinybackrooms::Engine::new(std::sync::Arc::new(OpenRouterConversant));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let response = runtime.block_on(engine.generate(&request))?;
    println!(
        "Retreat: {} real model calls, {} measured tokens, {} validated ranked items",
        response.usage.calls,
        response.usage.input_tokens + response.usage.output_tokens,
        response.items.len()
    );
    let decoded = rsi_ralph::decode_retreat_response(&serde_json::to_value(response)?)?;
    decoded
        .proposals
        .into_iter()
        .next()
        .ok_or_else(|| "retreat returned no valid typed proposal".into())
}
