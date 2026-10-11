//! Host-private immutable evaluation draws; debug output excludes their contents.
use crate::{Error, HostVerifier, Result, VerifierDeclaration, VerifierRegistry};
use std::collections::{BTreeMap, BTreeSet};
/// One immutable private task and its independent execution seeds.
///
/// No serialization or public prompt/checker accessor is provided.
#[derive(Clone)]
pub struct PrivateTask {
    pub(super) id: String,
    pub(super) family: String,
    pub(super) seat: String,
    pub(super) prompt: String,
    pub(super) verifier: HostVerifier,
    pub(super) seeds: Vec<u64>,
    pub(super) checker_stamp: String,
}
impl std::fmt::Debug for PrivateTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateTask").finish_non_exhaustive()
    }
}
impl PrivateTask {
    /// Freeze a task's actual prompt, strong checker and unique seeds.
    /// # Errors
    /// Rejects empty identities/prompts/seeds, duplicate seeds or invalid checkers.
    pub fn new(
        id: String,
        family: String,
        seat: String,
        prompt: String,
        verifier: HostVerifier,
        seeds: Vec<u64>,
    ) -> Result<Self> {
        if [&id, &family, &seat, &prompt]
            .iter()
            .any(|s| s.trim().is_empty())
            || seeds.is_empty()
            || seeds.iter().collect::<BTreeSet<_>>().len() != seeds.len()
        {
            return Err(Error::InvalidEvaluation);
        }
        VerifierRegistry::new(vec![VerifierDeclaration {
            node_id: "artifact".into(),
            owner_seat: seat.clone(),
            verifier: verifier.clone(),
        }])?;
        let checker_stamp = super::fingerprint::checker(&verifier)?;
        Ok(Self {
            id,
            family,
            seat,
            prompt,
            verifier,
            seeds,
            checker_stamp,
        })
    }
}
/// Four separate private stages, including a monitoring anchor draw.
#[derive(Clone)]
pub struct PrivateSuites {
    pub(super) draws: BTreeMap<String, Vec<PrivateTask>>,
    pub(super) fingerprints: BTreeMap<String, String>,
}
impl std::fmt::Debug for PrivateSuites {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateSuites")
            .field("fingerprints", &self.fingerprints)
            .finish_non_exhaustive()
    }
}
impl PrivateSuites {
    /// Freeze four disjoint task identities and seed sets.
    /// # Errors
    /// Rejects empty stages or task/seed reuse across stages.
    pub fn freeze(
        screen: Vec<PrivateTask>,
        selection: Vec<PrivateTask>,
        confirmation: Vec<PrivateTask>,
        anchor: Vec<PrivateTask>,
    ) -> Result<Self> {
        let draws: BTreeMap<String, Vec<PrivateTask>> = BTreeMap::from([
            ("screen".into(), screen),
            ("selection".into(), selection),
            ("confirmation".into(), confirmation),
            ("anchor".into(), anchor),
        ]);
        let mut ids = BTreeSet::new();
        let mut prior_seeds = BTreeSet::new();
        let mut fingerprints = BTreeMap::new();
        for (stage, tasks) in &draws {
            if tasks.is_empty() {
                return Err(Error::InvalidEvaluation);
            }
            let mut seeds = BTreeSet::new();
            for task in tasks {
                if !ids.insert(task.id.clone())
                    || task.seeds.iter().any(|seed| prior_seeds.contains(seed))
                {
                    return Err(Error::InvalidEvaluation);
                }
                seeds.extend(task.seeds.iter().copied());
            }
            prior_seeds.extend(seeds);
            fingerprints.insert(stage.clone(), super::fingerprint::suite(stage, tasks)?);
        }
        Ok(Self {
            draws,
            fingerprints,
        })
    }
    /// Public opaque identities; private prompts and checker inputs never escape.
    #[must_use]
    pub fn fingerprints(&self) -> BTreeMap<String, String> {
        self.fingerprints.clone()
    }
}
