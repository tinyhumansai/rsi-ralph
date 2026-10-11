//! The canned evolver replays in order and then reports exhaustion.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::*;

fn context() -> EvolveContext {
    EvolveContext {
        epoch: 1,
        index: 0,
        seed: 0,
    }
}

#[test]
fn replays_batches_in_order_then_runs_out() {
    let evolver: ScriptedEvolver<str, u32> = ScriptedEvolver::new([
        Proposed {
            tokens: 7,
            proposals: vec![Ok(1), Ok(2)],
        },
        Proposed {
            tokens: 0,
            proposals: vec![Err(Error::InvalidProposal)],
        },
    ]);
    let goal = Goal::new("g");
    assert_eq!(evolver.remaining(), 2);
    let first = evolver.propose("x", &goal, &context()).unwrap();
    assert_eq!((first.tokens, first.proposals), (7, vec![Ok(1), Ok(2)]));
    assert_eq!(
        evolver.propose("x", &goal, &context()).unwrap().proposals,
        vec![Err(Error::InvalidProposal)]
    );
    assert_eq!(
        evolver.propose("x", &goal, &context()),
        Err(Error::Exhausted)
    );
    assert_eq!(evolver.remaining(), 0);
}

#[test]
fn one_each_yields_a_single_free_proposal_per_call() {
    let evolver: ScriptedEvolver<str, &str> = ScriptedEvolver::one_each(["a", "b"]);
    let goal = Goal::new("g");
    for expected in ["a", "b"] {
        let proposed = evolver.propose("x", &goal, &context()).unwrap();
        assert_eq!(
            (proposed.tokens, proposed.proposals),
            (0, vec![Ok(expected)])
        );
    }
    assert!(format!("{evolver:?}").contains("ScriptedEvolver"));
}
