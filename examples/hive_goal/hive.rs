//! The hive under improvement: its package and goal, the founding archive
//! record, the host-private task suites, the scoring policy and the approval
//! gate.
//!
//! The hive has one seat (`solver`) on one desk (`work`). The host's
//! controlled runner binds exactly one task seat per hive, so the "agents"
//! here are every turn that seat takes; all of them read the same shared
//! context document, which is what the run improves.
use std::collections::BTreeMap;
use tinyhivemind_lang::lineage::{Direction, ObjectiveScore, Verdict, VersionMetadata};
use tinyhivemind_lang::{Goal, Role, Seat};
use tinyrsi_host::{
    Archive, Error, EvalDecision, FamilyAnchor, HApprovalGate, HostVerifier, Package, Patch,
    PatchClass, PrivateSuites, PrivateTask, Proposal, Result, ScoreComparison, ScorePenalties,
    ScoringPolicySpec,
};

/// The shared context document every agent reads.
pub(crate) const CONTEXT: &str = "context/method.md";
/// The stable item inside [`CONTEXT`] the repair edits.
pub(crate) const ITEM: &str = "method";
/// The seat that answers tasks.
pub(crate) const SEAT: &str = "solver";
/// The seat's prompt document (a different patch class than context).
pub(crate) const SEAT_PROMPT: &str = "seats/solver.md";
/// The body of [`SEAT_PROMPT`].
pub(crate) const SEAT_PROMPT_BODY: &str = "Follow the shared context.\n";
/// The desk the host routes work to.
pub(crate) const DESK: &str = "work";
/// The founding instruction: unhelpful, so the hive fails its tasks.
pub(crate) const UNHELPFUL: &str = "Do not work tasks out. Guess, and when unsure reply UNKNOWN.\n";
/// The repair. The offline agents answer correctly only once their shared
/// context carries this instruction.
pub(crate) const HELPFUL: &str =
    "Work out the arithmetic exactly and reply with only the requested result.\n";

/// The founding hive package: one seat, one desk, one goal, and a shared
/// context document whose only item is [`UNHELPFUL`]. Only context edits are
/// permitted.
pub(crate) fn package() -> Result<Package> {
    let mut p = Package::default();
    p.constitution.permitted_edits = vec![PatchClass::Context];
    p.manifest.goals.push(Goal {
        id: "solve".into(),
        description: "Answer every assigned arithmetic task exactly.".into(),
        acceptance: vec!["Host checkers pass every private task.".into()],
        subgoals: vec![],
    });
    p.manifest.context = vec![CONTEXT.into()];
    p.documents
        .insert(CONTEXT.into(), format!("<!-- item:{ITEM} -->\n{UNHELPFUL}"));
    p.manifest.roles.push(Role {
        id: "builder".into(),
        charter: "roles/builder.md".into(),
    });
    p.documents.insert(
        "roles/builder.md".into(),
        "Solve the assigned task and return only its requested artifact.\n".into(),
    );
    p.manifest.seats.push(Seat {
        id: SEAT.into(),
        label: "Solver".into(),
        role: "builder".into(),
        prompt: SEAT_PROMPT.into(),
        template: "artifact".into(),
        config: serde_json::json!({}),
        ..Default::default()
    });
    p.documents
        .insert(SEAT_PROMPT.into(), SEAT_PROMPT_BODY.into());
    p.manifest.desks.push(
        serde_json::from_value(serde_json::json!({
            "id": DESK, "name": "Work", "description": null, "members": [SEAT],
        }))
        .map_err(|_| Error::InvalidRecord)?,
    );
    Ok(p)
}

/// Records the founding package as the archive's accepted head and returns
/// its content id.
pub(crate) fn found(archive: &mut Archive) -> Result<String> {
    archive.record(
        package()?,
        VersionMetadata {
            parent: None,
            patch: Patch {
                class: PatchClass::Context,
                operations: vec![],
            },
            rationale: "founding hive with an unhelpful shared context".into(),
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

/// The public probe the live hive is sent each epoch. Its outcome is the
/// public failure evidence the proposer sees; it is not a private task.
pub(crate) fn probe() -> Result<PrivateTask> {
    PrivateTask::new(
        "public-probe".into(),
        "arithmetic".into(),
        SEAT.into(),
        "Compute 2 + 3.".into(),
        HostVerifier::Golden {
            expected: b"5".to_vec(),
        },
        vec![1],
    )
}

/// Two private tasks per stage, one per family: an exact sum checked by a
/// golden answer, and a product reported as JSON checked by a schema.
fn draw(stage: &str, seeds: [u64; 2], a: u64, b: u64) -> Result<Vec<PrivateTask>> {
    let product = a * b;
    Ok(vec![
        PrivateTask::new(
            format!("{stage}-sum"),
            "arithmetic".into(),
            SEAT.into(),
            format!("Compute {a} + {b}."),
            HostVerifier::Golden {
                expected: (a + b).to_string().into_bytes(),
            },
            seeds.to_vec(),
        )?,
        PrivateTask::new(
            format!("{stage}-product"),
            "report".into(),
            SEAT.into(),
            format!(
                "Compute {a} * {b} and report it as a JSON object with an integer field answer."
            ),
            HostVerifier::Schema {
                schema: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["answer"],
                    "properties": {"answer": {"const": product}},
                }),
            },
            seeds.to_vec(),
        )?,
    ])
}

/// Frozen host-private suites: screen, selection, confirmation and anchor
/// draws with disjoint tasks and seeds. Proposers never see them.
pub(crate) fn suites() -> Result<PrivateSuites> {
    PrivateSuites::freeze(
        draw("screen", [11, 12], 17, 25)?,
        draw("selection", [21, 22], 38, 47)?,
        draw("confirmation", [31, 32], 64, 29)?,
        draw("anchor", [41, 42], 12, 13)?,
    )
}

/// Normalized-H scoring: both task families anchored at 0 (nothing solved)
/// and 1 (everything solved), with a small practical margin.
pub(crate) fn policy() -> ScoringPolicySpec {
    ScoringPolicySpec {
        anchors: ["arithmetic", "report"]
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

/// The approval gate: this run is authorized to promote context edits whose
/// confirmed comparison accepted them, and nothing else. A production host
/// asks a human here.
#[derive(Debug)]
pub(crate) struct ContextOnlyApproval;

impl HApprovalGate for ContextOnlyApproval {
    fn approve_h(&mut self, _: &str, p: &Proposal, c: &ScoreComparison) -> Result<bool> {
        Ok(p.patch.class == PatchClass::Context
            && matches!(c.decision, EvalDecision::Accept { .. }))
    }
}
