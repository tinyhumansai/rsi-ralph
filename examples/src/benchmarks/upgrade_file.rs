//! File upgrade: bring `slug.py` to a written coding standard without
//! changing behavior. Fixtures live in `examples/upgrade_file/`.
use super::{Definition, metric};
use crate::{Grader, Replies, proposal, with_approach};
use tinyrsi::{Goal, LoopPolicy, MetricDirection};

const CHECKER: &str = include_str!("../../upgrade_file/check_standard.py");
const START: &str = include_str!("../../upgrade_file/slug.py");
const DOCSTRINGS: &str = include_str!("../../upgrade_file/proposals/docstrings.py");
const REGEX: &str = include_str!("../../upgrade_file/proposals/regex.py");
const STANDARD: &str = include_str!("../../upgrade_file/proposals/standard.py");

/// The written standard, as the judge and the supervisor read it.
fn standard() -> Goal {
    Goal::new("Bring slug.py up to the team's Python standard without changing behavior.")
        .with_criterion("the module and every function have a docstring")
        .with_criterion("every parameter and return value is type-annotated")
        .with_criterion("no bare except clauses")
        .with_criterion("no line is longer than 79 characters")
        .with_criterion("the code is readable without the original author")
}

fn brief(goal: &Goal) -> String {
    let mut text = format!(
        "{} The fixed checker check_standard.py runs behavior tests (slugify must keep \
         its exact output, including non-ASCII letters) and prints the number of standard \
         violations. Criteria:",
        goal.statement
    );
    for criterion in &goal.criteria {
        text.push_str("\n- ");
        text.push_str(criterion);
    }
    text
}

pub(super) fn definition() -> Definition {
    Definition {
        name: "slug.py",
        fixtures: &[("check_standard.py", CHECKER), ("slug.py", START)],
        editable: "slug.py",
        checker: "check_standard.py",
        brief: brief(&standard()),
        metric: metric("violations", MetricDirection::Minimize, 0.0),
        seeds: vec![1, 2],
        timeout_ms: 30_000,
        policy: LoopPolicy {
            max_epochs: 4,
            proposals_per_epoch: 1,
            patience: 3,
            ..LoopPolicy::default()
        },
    }
}

/// Docstrings, a regex rewrite that breaks Unicode slugs, then the full
/// upgrade.
pub(super) fn canned() -> Replies {
    Replies::new([
        with_approach(
            "docstrings",
            proposal(
                "The module and slugify are undocumented.",
                "Docstrings remove two violations.",
                "slug.py",
                DOCSTRINGS,
            ),
        ),
        with_approach(
            "regex rewrite",
            proposal(
                "A regex is shorter and fully annotated.",
                "One substitution satisfies the standard.",
                "slug.py",
                REGEX,
            ),
        ),
        with_approach(
            "full standard rewrite",
            proposal(
                "Annotate, drop the bare except, and build the slug from word runs.",
                "Every remaining violation goes, with behavior unchanged.",
                "slug.py",
                STANDARD,
            ),
        ),
    ])
}

/// The offline judge: one point per criterion it can see met in the source.
pub(super) fn grader() -> Grader {
    Grader::new(|prompt| {
        let source = prompt.split("# Subject").nth(1).unwrap_or_default();
        let checks = [
            source.trim_start().starts_with("\"\"\""),
            source.contains("->"),
            !source.contains("except:"),
            source.lines().all(|line| line.len() <= 79),
            source.matches("\"\"\"").count() >= 4,
        ];
        let met = checks.iter().filter(|ok| **ok).count();
        (
            f64::from(u8::try_from(met * 2).unwrap_or(10)),
            format!("{met} of 5 criteria visibly met"),
        )
    })
}
