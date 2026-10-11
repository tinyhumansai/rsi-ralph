//! Record extraction, repetition share, aggregation, rendering and the
//! `summary.json` shape.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use tinyrsi::{
    CandidateDecision, IncumbentEntry, IntervalUnits, LedgerCandidate, MetricDirection, MetricSpec,
    ScriptLedger,
};

fn row(
    epoch: u64,
    id: Option<&str>,
    hypothesis: &str,
    decision: CandidateDecision,
) -> LedgerCandidate {
    LedgerCandidate {
        epoch,
        proposal_id: (epoch > 0).then(|| format!("p{epoch}")),
        id: id.map(str::to_owned),
        parent: None,
        rationale: "why".into(),
        hypothesis: hypothesis.into(),
        paths: vec![],
        runs: vec![],
        mean: None,
        decision,
        estimate: None,
        lower: None,
        upper: None,
        units: IntervalUnits::Metric,
        promoted: matches!(decision, CandidateDecision::Baseline),
        note: None,
        approach: None,
        differs_from_previous: None,
        temperature: None,
    }
}

fn promoted(mut candidate: LedgerCandidate) -> LedgerCandidate {
    candidate.promoted = true;
    candidate
}

fn cell(benchmark: &str, model: &str, repeat: u32) -> RunCell {
    RunCell {
        benchmark: benchmark.into(),
        model: model.into(),
        repeat,
    }
}

/// Baseline; epoch 1 promotes `b` and rejects `c`; epoch 2 resubmits `c`
/// (duplicate), proposes something invalid, rewords `c`'s hypothesis, and
/// re-proposes the incumbent `b`.
fn ledger() -> ScriptLedger {
    ScriptLedger {
        run_id: "r".into(),
        subject: "s.py".into(),
        metric: MetricSpec {
            name: "colors".into(),
            direction: MetricDirection::Minimize,
            target: Some(6.0),
        },
        seeds: vec![1],
        tokens: 1234,
        stop: Some(StopReason::Stalled),
        incumbents: vec![
            IncumbentEntry {
                epoch: 0,
                id: "a".into(),
                metric: 17.0,
            },
            IncumbentEntry {
                epoch: 1,
                id: "b".into(),
                metric: 12.0,
            },
        ],
        candidates: vec![
            row(0, Some("a"), "starting script", CandidateDecision::Baseline),
            promoted(row(1, Some("b"), "DSATUR", CandidateDecision::Accept)),
            row(
                1,
                Some("c"),
                "Kempe chains reduce colours.",
                CandidateDecision::Reject,
            ),
            row(2, Some("c"), "something new", CandidateDecision::Duplicate),
            row(2, None, "escape the workspace", CandidateDecision::Invalid),
            row(
                2,
                Some("d"),
                "  kempe CHAINS -- reduce colours ",
                CandidateDecision::Inconclusive,
            ),
            row(2, Some("b"), "DSATUR", CandidateDecision::Duplicate),
        ],
    }
}

#[test]
fn hypotheses_normalize_case_punctuation_and_spacing() {
    assert_eq!(
        normalize_hypothesis("  Kempe-chains,  reduce   COLOURS! "),
        "kempe chains reduce colours"
    );
    assert_eq!(normalize_hypothesis(" -- "), "");
}

#[test]
fn repeats_count_content_and_hypothesis_matches_against_ruled_out_proposals() {
    let ledger = ledger();
    // Duplicate of rejected `c` and the reworded Kempe hypothesis repeat; the
    // invalid row is new; re-proposing the incumbent `b` is not a repeat.
    assert_eq!(repetition(&ledger.candidates), (2, 6));
}

#[test]
fn an_invalid_proposal_is_a_source_for_later_hypothesis_repeats() {
    let candidates = [
        row(1, None, "Escape", CandidateDecision::Invalid),
        row(2, None, "escape.", CandidateDecision::Invalid),
        row(2, Some("x"), "", CandidateDecision::Reject),
        row(3, Some("y"), "", CandidateDecision::Reject),
    ];
    // Empty hypotheses never match each other.
    assert_eq!(repetition(&candidates), (1, 4));
}

#[test]
fn a_run_without_proposals_has_no_repetition_share() {
    let mut ledger = ledger();
    ledger.candidates.truncate(1);
    ledger.incumbents.truncate(1);
    ledger.stop = Some(StopReason::GoalReached);
    let record = record(cell("b", "m", 1), Some(&ledger), 5, "dir".into(), None);
    assert_eq!(record.proposals, 0);
    assert_eq!(record.repetition_share, None);
    assert!(record.goal_reached);
    assert_eq!(record.epochs, 0);
    assert_eq!(record.final_metric, Some(17.0));
}

#[test]
fn records_carry_the_ledger_counts() {
    let record = record(
        cell("graph_coloring", "offline", 2),
        Some(&ledger()),
        42,
        "d".into(),
        None,
    );
    assert_eq!(record.benchmark, "graph_coloring");
    assert_eq!(record.repeat, 2);
    assert_eq!(record.stop, StopReason::Stalled);
    assert!(!record.goal_reached);
    assert_eq!(record.epochs, 2);
    assert_eq!(record.metric, "colors");
    assert_eq!(record.baseline_metric, Some(17.0));
    assert_eq!(record.final_metric, Some(12.0));
    assert_eq!(record.promotions, 1);
    assert_eq!(record.proposals, 6);
    assert_eq!(record.rejected, 5);
    assert_eq!(record.duplicates, 2);
    assert_eq!(record.invalid, 1);
    assert_eq!(record.repeats, 2);
    assert_eq!(record.repetition_share, Some(2.0 / 6.0));
    assert_eq!(record.tokens, 1234);
    assert_eq!(record.wall_ms, 42);
    assert_eq!(record.error, None);
}

#[test]
fn a_failed_run_is_recorded_as_an_error() {
    let record = record(cell("b", "m", 1), None, 7, "d".into(), Some("boom".into()));
    assert_eq!(record.stop, StopReason::Error);
    assert!(!record.goal_reached);
    assert_eq!(record.error.as_deref(), Some("boom"));
    assert_eq!(record.final_metric, None);
    assert_eq!(record.proposals, 0);
    // A ledger without a stop reason (the run died mid-epoch) is an error too.
    let mut ledger = ledger();
    ledger.stop = None;
    let record = super::record(cell("b", "m", 1), Some(&ledger), 7, "d".into(), None);
    assert_eq!(record.stop, StopReason::Error);
}

#[test]
fn model_labels_are_safe_directory_names() {
    assert_eq!(
        model_label("anthropic/claude-sonnet-4.5"),
        "anthropic_claude-sonnet-4.5"
    );
    assert_eq!(
        model_label("openai/gpt-4.1-mini:free"),
        "openai_gpt-4.1-mini_free"
    );
    assert_eq!(model_label("offline"), "offline");
}

fn finished(
    model: &str,
    repeat: u32,
    goal: bool,
    epochs: u64,
    tokens: u64,
    share: Option<f64>,
) -> RunRecord {
    let mut ledger = ledger();
    ledger.stop = Some(if goal {
        StopReason::GoalReached
    } else {
        StopReason::Stalled
    });
    ledger.tokens = tokens;
    let mut record = record(
        cell("graph_coloring", model, repeat),
        Some(&ledger),
        1000 * u64::from(repeat),
        "d".into(),
        None,
    );
    record.epochs = epochs;
    record.repetition_share = share;
    record
}

#[test]
fn aggregation_groups_cells_in_first_seen_order() {
    let records = [
        finished("m1", 1, true, 3, 100, Some(0.5)),
        finished("m1", 2, false, 6, 300, None),
        finished("m1", 3, true, 2, 200, Some(0.0)),
        finished("m2", 1, true, 4, 10, Some(0.25)),
        finished("m2", 2, true, 2, 20, Some(0.25)),
    ];
    let rows = aggregate(&records);
    assert_eq!(rows.len(), 2);
    let m1 = &rows[0];
    assert_eq!((m1.model.as_str(), m1.runs, m1.goals), ("m1", 3, 2));
    assert_eq!(m1.metric, "colors");
    assert!((m1.goal_rate - 2.0 / 3.0).abs() < 1e-12);
    // Median of the goal runs' epochs (3, 2) is their mean.
    assert_eq!(m1.median_epochs_to_goal, Some(2.5));
    assert_eq!(m1.median_tokens, Some(200.0));
    assert_eq!(m1.median_final_metric, Some(12.0));
    assert_eq!(m1.mean_repetition_share, Some(0.25));
    assert_eq!(m1.median_wall_ms, Some(2000.0));
    let m2 = &rows[1];
    assert_eq!(m2.median_epochs_to_goal, Some(3.0));
    assert_eq!(m2.median_tokens, Some(15.0));
}

#[test]
fn a_cell_without_goals_or_metrics_aggregates_to_nulls() {
    let failed = record(cell("b", "m", 1), None, 0, "d".into(), Some("x".into()));
    let rows = aggregate(&[failed]);
    let row = &rows[0];
    assert_eq!((row.goals, row.goal_rate), (0, 0.0));
    assert_eq!(row.median_epochs_to_goal, None);
    assert_eq!(row.median_final_metric, None);
    assert_eq!(row.mean_repetition_share, None);
    assert_eq!(row.metric, "");
    assert_eq!(aggregate(&[]), Vec::<SummaryRow>::new());
}

#[test]
fn the_table_renders_rates_medians_and_dashes() {
    let rows = aggregate(&[
        finished("m1", 1, true, 3, 1500, Some(0.5)),
        finished("m1", 2, false, 6, 2500, None),
    ]);
    let failed = aggregate(&[record(
        cell("b", "m", 1),
        None,
        0,
        "d".into(),
        Some("x".into()),
    )]);
    let table = render_markdown(&[rows, failed].concat());
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(
        lines[0],
        "| Benchmark | Model | Goal | Epochs to goal | Tokens | Final metric | Repetition | Wall (s) |"
    );
    assert_eq!(
        lines[1],
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |"
    );
    assert_eq!(
        lines[2],
        "| graph_coloring | m1 | 1/2 | 3 | 2000 | 12 colors | 50% | 1.5 |"
    );
    assert_eq!(lines[3], "| b | m | 0/1 | – | 0 | – | – | 0.0 |");
    assert_eq!(lines.len(), 4);
}

#[test]
fn summary_json_has_the_documented_shape() {
    let runs = vec![finished("m1", 1, true, 3, 100, Some(0.5))];
    let summary = Summary {
        rows: aggregate(&runs),
        runs,
    };
    let value = serde_json::to_value(&summary).unwrap();
    let keys = |v: &serde_json::Value| {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(keys(&value), ["rows", "runs"]);
    assert_eq!(
        keys(&value["rows"][0]),
        [
            "benchmark",
            "goal_rate",
            "goals",
            "mean_repetition_share",
            "median_epochs_to_goal",
            "median_final_metric",
            "median_tokens",
            "median_wall_ms",
            "metric",
            "model",
            "runs",
        ]
    );
    assert_eq!(
        keys(&value["runs"][0]),
        [
            "baseline_metric",
            "benchmark",
            "duplicates",
            "epochs",
            "error",
            "final_metric",
            "goal_reached",
            "invalid",
            "metric",
            "model",
            "promotions",
            "proposals",
            "rejected",
            "repeat",
            "repeats",
            "repetition_share",
            "run_dir",
            "stop",
            "tokens",
            "wall_ms",
        ]
    );
    assert_eq!(value["runs"][0]["stop"], "goal_reached");
    assert_eq!(value["rows"][0]["goal_rate"], 1.0);
    let back: Summary = serde_json::from_value(value).unwrap();
    assert_eq!(back, summary);
}
