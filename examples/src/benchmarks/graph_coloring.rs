//! Graph colouring: colour random graphs with a hidden 6-colouring using as
//! few colours as possible. Fixtures live in `examples/graph_coloring/`.
use super::{Definition, metric};
use crate::{Replies, proposal};
use tinyrsi::{LoopPolicy, MetricDirection};

const CHECKER: &str = include_str!("../../graph_coloring/check_coloring.py");
const START: &str = include_str!("../../graph_coloring/coloring.py");
const DSATUR: &str = include_str!("../../graph_coloring/proposals/dsatur.py");
const RANDOM_ORDER: &str = include_str!("../../graph_coloring/proposals/random_order.py");
const TABUCOL: &str = include_str!("../../graph_coloring/proposals/tabucol.py");

const BRIEF: &str = "coloring.py defines color(n, edges): given n vertices labelled 0..n-1 and a \
list of undirected edges (a, b), return a list of n non-negative integers so that no edge joins \
two vertices of the same colour. The fixed checker check_coloring.py feeds it random graphs with \
150 vertices and about 3,300 edges, rejects any invalid colouring (the seed then fails), and \
prints the number of distinct colours used. Rewrite coloring.py so color() uses as few colours \
as possible. Use only the Python standard library; stay well under 20 seconds per graph; do not \
read files, the environment, or the checker. Name the algorithm in a docstring. Print \
`DIAG <text>` lines to stdout to report what your algorithm did (for example how many \
colours each phase reached, moves tried, conflicts left); you will see them next round.";

pub(super) fn definition() -> Definition {
    Definition {
        name: "coloring.py",
        fixtures: &[("check_coloring.py", CHECKER), ("coloring.py", START)],
        editable: "coloring.py",
        checker: "check_coloring.py",
        brief: BRIEF.into(),
        metric: metric("colors", MetricDirection::Minimize, 6.0),
        seeds: vec![1, 2, 3],
        timeout_ms: 30_000,
        policy: LoopPolicy {
            max_epochs: 6,
            proposals_per_epoch: 2,
            patience: 3,
            ..LoopPolicy::default()
        },
    }
}

/// DSATUR (better), a buggy shuffled first fit (invalid colourings,
/// rejected), then DSATUR with `TabuCol` (reaches 6).
pub(super) fn canned() -> Replies {
    Replies::new([
        proposal(
            "First fit in label order ignores structure; constrained vertices get colours late.",
            "DSATUR colours the most saturated vertex first and needs fewer colours.",
            "coloring.py",
            DSATUR,
        ),
        proposal(
            "Label order is arbitrary, so a shuffled order should be no worse.",
            "First fit in a random order matches DSATUR at lower cost.",
            "coloring.py",
            RANDOM_ORDER,
        ),
        proposal(
            "DSATUR is greedy and stops at its first valid colouring.",
            "TabuCol local search shrinks the DSATUR palette to the optimum.",
            "coloring.py",
            TABUCOL,
        ),
    ])
}
