//! Persisted archive events and proposal audit evidence.
use serde::{Deserialize, Serialize};
use tinyhivemind_lang::Package;
use tinyhivemind_lang::lineage::Record;
use tinyhivemind_lang::patch::Patch;

/// An attempted proposal that did not produce an evaluated language record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalEvent {
    /// Host epoch that attempted this proposal.
    pub epoch: u64,
    /// Exact attempted patch, including invalid operations.
    pub patch: Patch,
    /// Proposer's explanation.
    pub rationale: String,
    /// Host reason for rejecting or guarding the attempt.
    pub reason: String,
    /// Public transcript sequence references supporting the attempt.
    pub transcript_refs: Vec<u64>,
}

/// Exact proposal inputs committed before executing a candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalDeclaration {
    /// Host epoch that issued the declaration.
    pub epoch: u64,
    /// Original typed proposal, including prediction, risk and effort.
    pub proposal: crate::Proposal,
}

/// Exact paired host outcomes retained separately from summary objectives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationEvent {
    /// Content identity of the evaluated candidate, possibly recorded later.
    pub candidate_id: String,
    /// Host stage, such as screening, selection, or confirmation.
    pub stage: String,
    /// Incumbent outcomes for the identical tasks and seeds.
    pub incumbent: Vec<crate::EvalRun>,
    /// Candidate outcomes for the identical tasks and seeds.
    pub candidate: Vec<crate::EvalRun>,
    /// Hard per-run cap frozen for this comparison; absent in legacy evidence.
    #[serde(default)]
    pub budget: Option<crate::Budget>,
    /// Exact statistical policy, including any prediction-adjusted margin.
    #[serde(default)]
    pub policy: Option<crate::EvalPolicy>,
    /// Host comparison recomputed and verified when the archive is replayed.
    #[serde(default)]
    pub comparison: Option<crate::Comparison>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Event {
    Candidate {
        record: Box<Record>,
        package: Box<Package>,
    },
    Prepared {
        id: String,
        package: Box<Package>,
        watermarks: std::collections::BTreeMap<String, u64>,
    },
    Activation {
        id: String,
    },
    Proposal {
        proposal: ProposalEvent,
    },
    Declaration {
        declaration: ProposalDeclaration,
    },
    Evaluation {
        evaluation: EvaluationEvent,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snapshot {
    pub version: u64,
    pub events: Vec<Event>,
}
