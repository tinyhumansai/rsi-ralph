//! The port's value types.
use super::*;

#[test]
fn proposals_compare_by_tokens_and_contents() {
    let a: Proposed<u8> = Proposed {
        tokens: 3,
        proposals: vec![Ok(1), Err(crate::Error::InvalidProposal)],
    };
    let b = Proposed {
        tokens: 3,
        proposals: vec![Ok(1), Err(crate::Error::InvalidProposal)],
    };
    assert_eq!(a, b);
    let context = EvolveContext {
        epoch: 2,
        index: 1,
        seed: 5,
    };
    assert_eq!(context, context.clone());
    assert!(format!("{a:?}").contains("tokens: 3"));
}
