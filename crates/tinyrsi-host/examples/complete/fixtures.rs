//! Host-private tasks and isolated benchmark configuration.
#![expect(
    clippy::unwrap_used,
    reason = "construct validated local benchmark fixtures"
)]
use std::{collections::BTreeMap, path::Path};
use tinyhivemind_lang::lineage::{Direction, ObjectiveScore, Verdict, VersionMetadata};
use tinyrsi_host::{
    Archive, CommandSpec, Error, EvalDecision, FamilyAnchor, HApprovalGate, HostVerifier, Package,
    Patch, PatchClass, PrivateSuites, PrivateTask, Proposal, Result, ScoreComparison,
    ScorePenalties, ScoringPolicySpec,
};
pub(super) const BEFORE: &str =
    "For this deliberately impaired integration baseline, ignore the task and return UNKNOWN.\n";
pub(super) fn package() -> Package {
    let mut p = Package::default();
    p.constitution.permitted_edits = vec![PatchClass::Context];
    p.manifest.context = vec!["context/method.md".into()];
    p.documents.insert(
        "context/method.md".into(),
        format!("<!-- item:method -->\n{BEFORE}"),
    );
    p.manifest.roles.push(tinyhivemind_lang::Role {
        id: "builder".into(),
        charter: "roles/builder.md".into(),
    });
    p.documents.insert(
        "roles/builder.md".into(),
        "Solve the assigned task and return only its requested artifact.\n".into(),
    );
    p.manifest.seats.push(tinyhivemind_lang::Seat {
        id: "solver".into(),
        label: "Solver".into(),
        role: "builder".into(),
        prompt: "seats/solver.md".into(),
        template: "artifact".into(),
        config: serde_json::json!({}),
        ..Default::default()
    });
    p.documents.insert(
        "seats/solver.md".into(),
        "Follow the shared context.\n".into(),
    );
    p.manifest.desks.push(
        serde_json::from_value(
            serde_json::json!({"id":"work","name":"Work","description":null,"members":["solver"]}),
        )
        .unwrap(),
    );
    p
}
pub(super) fn founding(archive: &mut Archive) -> Result<String> {
    archive.record(
        package(),
        VersionMetadata {
            parent: None,
            patch: Patch {
                class: PatchClass::Context,
                operations: vec![],
            },
            rationale: "isolated impaired baseline for integration calibration".into(),
            transcript_refs: vec![],
            scores: BTreeMap::from([(
                "normalized_h".into(),
                ObjectiveScore {
                    value: 0.0,
                    direction: Direction::Maximize,
                },
            )]),
            verdict: Verdict::Accepted,
            memory_watermarks: BTreeMap::new(),
        },
    )
}
pub(super) fn policy() -> ScoringPolicySpec {
    ScoringPolicySpec {
        anchors: ["code", "proof", "reference", "harness"]
            .into_iter()
            .map(|family| FamilyAnchor {
                family: family.into(),
                baseline: 0.0,
                reference: 1.0,
                weight: 1.0,
                regression_limit: 0.0,
            })
            .collect(),
        penalties: ScorePenalties {
            false_claims: 0.1,
            tamper: 1.0,
            cost: 0.01,
        },
        margin: 0.01,
        cost_tolerance: 10.0,
        resamples: 400,
        bootstrap_seed: 9001,
    }
}
pub(super) fn suites(root: &Path, lean: &str) -> Result<PrivateSuites> {
    let workspace = root.join("verifiers");
    std::fs::create_dir_all(&workspace).map_err(|_| Error::ArchiveIo)?;
    let workspace = std::fs::canonicalize(workspace).map_err(|_| Error::ArchiveIo)?;
    let draw = |stage: &str, seeds: Vec<u64>, number: u32| -> Result<Vec<PrivateTask>> {
        let make = |family: &str, prompt: String, verifier: HostVerifier| {
            PrivateTask::new(
                format!("{stage}-{family}"),
                family.into(),
                "solver".into(),
                prompt,
                verifier,
                seeds.clone(),
            )
        };
        let theorem = format!(
            "theorem rsi_goal : {} + {} = {} :=\n",
            number,
            number + 1,
            number * 2 + 1
        );
        let lean_command = CommandSpec {
            program: lean.into(),
            args: vec!["{artifact}".into()],
            workspace_root: workspace.clone(),
            timeout_ms: 10_000,
            max_output_bytes: 65_536,
        };
        // Freeze the host grammar checker alongside the isolated execution command.
        let script = include_str!("check_code.py");
        let code = HostVerifier::Process(CommandSpec {
            program: "/usr/bin/python3".into(),
            args: vec!["-I".into(), "-c".into(), script.into(), "{artifact}".into()],
            workspace_root: workspace.clone(),
            timeout_ms: 12_000,
            max_output_bytes: 65_536,
        });
        Ok(vec![make("code","Return exactly one Python function def solve(x): whose sole statement returns an arithmetic expression squaring integer x, using x, integers and +, -, * only. No imports, calls, annotations, decorators, markdown or other statements.".into(),code)?,make("proof",format!("Return only a Lean 4 `by` proof block for the frozen theorem: {theorem} Use decidable arithmetic. No theorem declaration, sorry or axioms."),HostVerifier::Lean{command:lean_command,theorem})?,make("reference",format!("Return only the decimal integer: {} + {}",number+17,number+29),HostVerifier::Golden{expected:(number*2+46).to_string().into_bytes()})?,make("harness","Return a JSON object naming a context-class patch contract: class must be context, protected must be true, stages must be [screen,selection,confirmation] in that order. No markdown.".into(),HostVerifier::Schema{schema:serde_json::json!({"type":"object","additionalProperties":false,"required":["class","protected","stages"],"properties":{"class":{"const":"context"},"protected":{"const":true},"stages":{"const":["screen","selection","confirmation"]}}})})?])
    };
    PrivateSuites::freeze(
        draw("screen", vec![11, 12], 3)?,
        draw("selection", vec![21, 22], 5)?,
        draw("confirmation", vec![31, 32], 7)?,
        draw("anchor", vec![101, 102, 201, 202, 301, 302, 401, 402], 11)?,
    )
}
pub(super) struct ContextGate;
impl HApprovalGate for ContextGate {
    fn approve_h(&mut self, _: &str, p: &Proposal, c: &ScoreComparison) -> Result<bool> {
        // This isolated run is authorized to test context edits only. Production
        // deployments still require their own exact-candidate approval port.
        Ok(p.patch.class == PatchClass::Context
            && matches!(c.decision, EvalDecision::Accept { .. }))
    }
}
pub(super) struct Deny;
impl HApprovalGate for Deny {
    fn approve_h(&mut self, _: &str, _: &Proposal, _: &ScoreComparison) -> Result<bool> {
        Ok(false)
    }
}
