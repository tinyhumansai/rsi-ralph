//! Math problem: compute pi to 12 digits. Fixtures live in
//! `examples/math_problem/`.
use super::{Definition, metric};
use crate::{Grader, Replies, proposal, with_approach};
use rsi_ralph::{LoopPolicy, MetricDirection};

const CHECKER: &str = include_str!("../../math_problem/check_pi.py");
const START: &str = include_str!("../../math_problem/pi.py");
const MORE_TERMS: &str = include_str!("../../math_problem/proposals/more_terms.py");
const WALLIS: &str = include_str!("../../math_problem/proposals/wallis.py");
const MACHIN: &str = include_str!("../../math_problem/proposals/machin.py");

const BRIEF: &str = "pi.py defines estimate(), which must return pi as a float. The fixed \
checker check_pi.py prints how many decimal digits are correct. Rewrite pi.py so estimate() \
is correct to at least 12 digits, using only the Python standard library and no stored \
constant (math.pi is not allowed). Name the method in a docstring and justify each step.";

pub(super) fn definition() -> Definition {
    Definition {
        name: "pi.py",
        fixtures: &[("check_pi.py", CHECKER), ("pi.py", START)],
        editable: "pi.py",
        checker: "check_pi.py",
        brief: BRIEF.into(),
        metric: metric("digits", MetricDirection::Maximize, 12.0),
        seeds: vec![1, 2, 3],
        timeout_ms: 30_000,
        policy: LoopPolicy {
            max_epochs: 4,
            proposals_per_epoch: 1,
            patience: 3,
            ..LoopPolicy::default()
        },
    }
}

/// More terms, a slower product, then Machin's formula.
pub(super) fn canned() -> Replies {
    Replies::new([
        with_approach(
            "Leibniz series",
            proposal(
                "The Leibniz series converges slowly; 1,000 terms leave an error near 1e-3.",
                "200,000 terms gain about two more digits.",
                "pi.py",
                MORE_TERMS,
            ),
        ),
        with_approach(
            "Wallis product",
            proposal(
                "A product converges differently from a series; try Wallis.",
                "The Wallis product reaches more digits than the series.",
                "pi.py",
                WALLIS,
            ),
        ),
        with_approach(
            "Machin arctan formula",
            proposal(
                "Machin's arctan identity converges geometrically.",
                "Twenty arctan terms reach full double precision.",
                "pi.py",
                MACHIN,
            ),
        ),
    ])
}

/// The offline judge: it rewards a named, justified method.
pub(super) fn grader() -> Grader {
    Grader::new(|prompt| {
        let subject = prompt.split("# Subject").nth(1).unwrap_or_default();
        let named = ["Leibniz", "Wallis", "Machin"]
            .iter()
            .any(|m| subject.contains(m));
        let justified = subject.matches("\"\"\"").count() >= 4 || subject.contains("identity");
        match (named, justified) {
            (true, true) => (9.0, "names its method and justifies each step".into()),
            (true, false) => (6.0, "names its method but justifies little".into()),
            _ => (2.0, "no method named, no justification".into()),
        }
    })
}
