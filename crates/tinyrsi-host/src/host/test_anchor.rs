//! Anchor regression enforcement: automatic rollback, revocation and strike.
use super::*;
use crate::{AnchorSignal, PatchShape, PenaltyPolicy, RevocationEvent, RevocationReason};

fn signal(regression: bool, widening_gap: bool) -> AnchorSignal {
    AnchorSignal {
        regression,
        widening_gap,
        best_epoch: 1,
    }
}
fn host() -> CoordinatorHost {
    CoordinatorHost::new(
        "runtime".into(),
        Arc::new(MemoryStorage::new()),
        Arc::new(Factory::default()),
        Arc::new(Memory),
    )
}
/// A deployed child of the baseline, observed once so the store is at epoch 1.
async fn deployed_child(
    dir: &Path,
    archive: &mut Archive,
    host: &mut CoordinatorHost,
) -> (DeploymentStore, String, String) {
    let base = archive.head().unwrap().id.clone();
    child(archive);
    let target = archive.head().unwrap().id.clone();
    let mut store = DeploymentStore::open(dir.join("deployment")).unwrap();
    store.activate(archive, host).await.unwrap();
    store.observe(host.coordinator().unwrap()).unwrap();
    (store, base, target)
}

#[tokio::test]
async fn an_anchor_regression_rolls_back_and_cools_the_deployed_shape() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let mut host = host();
    let (mut store, base, target) = deployed_child(dir.path(), &mut archive, &mut host).await;
    assert!(
        store
            .enforce_anchor(&signal(true, true), &mut archive, &mut host)
            .await
            .unwrap()
    );
    assert_eq!(store.checkpoint().deployed_id, Some(base.clone()));
    assert_eq!(archive.head().unwrap().id, base);
    assert_eq!(
        archive.revocations(),
        vec![&RevocationEvent {
            candidate_id: target.clone(),
            epoch: 1,
            reason: RevocationReason::AnchorRegression,
        }]
    );
    let record = archive.records().iter().find(|r| r.id == target).unwrap();
    let shape = PatchShape::of(&record.patch);
    let strikes = archive.penalties().strikes;
    assert_eq!(strikes.len(), 1);
    assert_eq!(strikes[0].kind, crate::StrikeKind::Revoked);
    assert_eq!(strikes[0].shape, shape);
    // A same-shape proposal in the next epoch is cooled before any evaluation.
    assert!(
        archive
            .penalties()
            .cooled(&shape, 2, &PenaltyPolicy::default())
    );
}
#[tokio::test]
async fn a_widening_gap_alone_or_a_quiet_signal_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let mut host = host();
    let (mut store, _, target) = deployed_child(dir.path(), &mut archive, &mut host).await;
    for quiet in [signal(false, true), signal(false, false)] {
        assert!(
            !store
                .enforce_anchor(&quiet, &mut archive, &mut host)
                .await
                .unwrap()
        );
    }
    assert_eq!(store.checkpoint().deployed_id, Some(target.clone()));
    assert_eq!(archive.head().unwrap().id, target);
    assert_eq!(archive.revocations(), Vec::<&RevocationEvent>::new());
    assert_eq!(archive.penalties().strikes, Vec::new());
}
#[tokio::test]
async fn anchor_enforcement_propagates_rollback_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut archive = archive(&dir.path().join("archive"));
    let mut host = host();
    let mut store = DeploymentStore::open(dir.path().join("deployment")).unwrap();
    store.activate(&archive, &mut host).await.unwrap();
    assert_eq!(
        store
            .enforce_anchor(&signal(true, false), &mut archive, &mut host)
            .await,
        Err(Error::NoParent)
    );
    let base = archive.head().unwrap().id.clone();
    child(&mut archive);
    let mut checkpoint = store.state.clone();
    checkpoint.pending = Some(DeploymentIntent {
        from: Some(base),
        to: archive.head().unwrap().id.clone(),
        kind: DeploymentKind::Activate,
        applied: false,
    });
    store.save(checkpoint).unwrap();
    assert_eq!(
        store
            .enforce_anchor(&signal(true, false), &mut archive, &mut host)
            .await,
        Err(Error::HostFailed)
    );
    assert_eq!(archive.revocations(), Vec::<&RevocationEvent>::new());
}
