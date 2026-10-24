//! JSON persistence preserves every bit needed to recompute paired comparisons.
#![expect(clippy::unwrap_used, reason = "assert test setup and outcomes")]
use super::*;

#[test]
fn reopens_exact_comparison_with_nontrivial_float_roundtrip() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("archive.json");
    let mut archive = Archive::open(&path).unwrap();
    let id = archive
        .prepare(Package::default(), BTreeMap::new())
        .unwrap();
    let incumbent: Vec<_> = [1, 2, 3]
        .into_iter()
        .map(|seed| crate::EvalRun {
            task: "task".into(),
            family: "code".into(),
            seed,
            quality: 0.1,
            tokens: 1,
            false_claims: 0,
            tamper: 0,
        })
        .collect();
    let mut candidate = incumbent.clone();
    for (run, quality) in candidate.iter_mut().zip([
        0.180_150_851_860_176_76,
        0.372_544_340_695_013_1,
        0.884_373_434_507_995_7,
    ]) {
        run.quality = quality;
    }
    let budget = crate::Budget { max_tokens: 10 };
    let policy = crate::EvalPolicy::default();
    let comparison = crate::compare(&incumbent, &candidate, &budget, &policy).unwrap();
    let event = EvaluationEvent {
        candidate_id: id,
        stage: "Selection".into(),
        incumbent,
        candidate,
        budget: Some(budget),
        policy: Some(policy),
        comparison: Some(comparison),
    };
    archive.record_evaluation(event.clone()).unwrap();
    drop(archive);
    let archive = Archive::open(path).unwrap();
    assert_eq!(archive.evaluations(), vec![&event]);
}
