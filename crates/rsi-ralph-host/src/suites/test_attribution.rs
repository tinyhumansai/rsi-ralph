//! Paired runs blame the candidate only when the incumbent completed its draw.
use super::*;
use crate::{CandidateFailure, CandidateFailureKind, ModelError};

/// Agents that misbehave when their seat prompt carries a marker.
struct Marked {
    calls: AtomicUsize,
}
impl ModelPort for Marked {
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let system = request.system().to_owned();
        Box::pin(async move {
            if system.contains("OUTAGE") {
                return Err(ModelError::CallFailed);
            }
            if system.contains("REFUSE") {
                return Err(ModelError::InvalidRequest);
            }
            if system.contains("SLOW") {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
            Ok(ModelReply {
                content: b"good".to_vec(),
                input_tokens: 10,
                output_tokens: if system.contains("VERBOSE") { 1000 } else { 2 },
            })
        })
    }
}
fn marked(marker: &str) -> CandidateState {
    let mut state = state();
    state
        .package
        .documents
        .insert("seats/alice.md".into(), format!("Solve carefully {marker}"));
    state
}
fn evaluator(timeout_ms: u64) -> (CoordinatorEvaluator, Arc<Marked>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let memory = SnapshotMemoryStore::open(dir.path().join("memory"), vec![], 100_000).unwrap();
    let model = Arc::new(Marked {
        calls: AtomicUsize::new(0),
    });
    let evaluator =
        CoordinatorEvaluator::new(suites(), model.clone(), memory, 100, timeout_ms).unwrap();
    (evaluator, model, dir)
}
fn screen(
    evaluator: &mut CoordinatorEvaluator,
    incumbent: &CandidateState,
    candidate: &CandidateState,
) -> Result<HEvalPair> {
    evaluator.evaluate_h(
        incumbent,
        candidate,
        EvalStage::Screen,
        &Budget { max_tokens: 10_000 },
    )
}
fn failed(kind: CandidateFailureKind, failed_runs: u32) -> Error {
    Error::CandidateFailed(CandidateFailure {
        kind,
        failed_runs,
        runs: 2,
    })
}

#[test]
fn a_candidate_only_crash_is_attributed_to_the_candidate() {
    let crash = runner::Blame::Hive(CandidateFailureKind::Crash);
    assert_eq!(
        charge(&[crash, crash], 2).unwrap(),
        Some(failed(CandidateFailureKind::Crash, 2))
    );
    assert_eq!(charge(&[], 2).unwrap(), None);
}

#[test]
fn any_host_owned_failure_makes_the_draw_a_host_failure() {
    let crash = runner::Blame::Hive(CandidateFailureKind::Crash);
    assert_eq!(
        charge(&[crash, runner::Blame::Host], 2).unwrap(),
        Some(Error::HostFailed)
    );
}

#[test]
fn a_provider_outage_on_the_candidate_arm_stays_a_host_failure() {
    let (mut evaluator, model, _dir) = evaluator(1000);
    let outcome = screen(&mut evaluator, &state(), &marked("OUTAGE"));
    assert_eq!(outcome.unwrap_err(), Error::HostFailed);
    assert_eq!(model.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn an_incumbent_crash_stays_a_host_failure_and_never_runs_the_candidate() {
    for candidate in [state(), marked("OUTAGE")] {
        let (mut evaluator, model, _dir) = evaluator(1000);
        let outcome = screen(&mut evaluator, &marked("OUTAGE"), &candidate);
        assert_eq!(outcome.unwrap_err(), Error::HostFailed);
        assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    }
}

#[test]
fn candidate_timeouts_and_overspent_replies_are_classified() {
    for (marker, kind) in [
        ("SLOW", CandidateFailureKind::Timeout),
        ("VERBOSE", CandidateFailureKind::OverBudget),
        ("REFUSE", CandidateFailureKind::OverBudget),
    ] {
        let (mut evaluator, _, _dir) = evaluator(200);
        let outcome = screen(&mut evaluator, &state(), &marked(marker));
        assert_eq!(outcome.unwrap_err(), failed(kind, 2), "{marker}");
    }
}

#[test]
fn an_unsupported_or_unreservable_candidate_fails_before_any_model_call() {
    let mut unsupported = state();
    unsupported
        .package
        .manifest
        .mcp
        .push(tinyhivemind_lang::McpServer {
            id: "external".into(),
            endpoint: "external-mcp".into(),
            ..Default::default()
        });
    unsupported.package.manifest.seats[0]
        .mcp
        .push("external".into());
    let unreservable = marked(&"x".repeat(20_000));
    for (candidate, kind) in [
        (&unsupported, CandidateFailureKind::Unsupported),
        (&unreservable, CandidateFailureKind::OverBudget),
    ] {
        let (mut evaluator, model, _dir) = evaluator(1000);
        let outcome = screen(&mut evaluator, &state(), candidate);
        assert_eq!(outcome.unwrap_err(), failed(kind, 2));
        assert_eq!(model.calls.load(Ordering::SeqCst), 0);
    }
    let (mut evaluator, model, _dir) = evaluator(1000);
    assert_eq!(
        screen(&mut evaluator, &unsupported, &state()).unwrap_err(),
        Error::HostFailed
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
}
