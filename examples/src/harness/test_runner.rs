//! Matrix planning, model choice, and an offline matrix end to end.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::runner::{ModelChoice, execute, plan, run_job, write_summary};
use super::{Summary, aggregate};
use crate::benchmarks::Benchmark;
use crate::python;
use std::path::Path;
use std::sync::Mutex;
use rsi_ralph::StopReason;

#[test]
fn the_plan_nests_benchmark_model_repeat_with_one_directory_each() {
    let models = [
        ModelChoice::Offline,
        ModelChoice::Live("anthropic/claude-sonnet-4.5".into()),
    ];
    let jobs = plan(
        &[Benchmark::CurveFit, Benchmark::MathProblem],
        &models,
        2,
        Path::new("out"),
    );
    assert_eq!(jobs.len(), 8);
    let cells: Vec<String> = jobs.iter().map(|j| j.run.id.clone()).collect();
    assert_eq!(cells[0], "curve_fit-offline-1");
    assert_eq!(cells[3], "curve_fit-anthropic_claude-sonnet-4.5-2");
    assert_eq!(cells[4], "math_problem-offline-1");
    assert_eq!(
        jobs[3].run.dir,
        Path::new("out/curve_fit/anthropic_claude-sonnet-4.5/2")
    );
}

#[test]
fn live_choices_build_a_shared_supervisor_and_judge_without_calling_it() {
    let models = ModelChoice::Live("openai/gpt-4.1-mini".into())
        .models(Benchmark::GraphColoring)
        .unwrap();
    assert!(models.judge.is_some());
    assert!(
        ModelChoice::Live(String::new())
            .models(Benchmark::CurveFit)
            .is_err()
    );
    assert_eq!(ModelChoice::Offline.label(), "offline");
}

#[test]
fn a_failing_run_is_recorded_without_stopping() {
    let dir = tempfile::tempdir().unwrap();
    let jobs = plan(
        &[Benchmark::MathProblem],
        &[ModelChoice::Offline],
        1,
        dir.path(),
    );
    let record = run_job(&jobs[0], Path::new("/nonexistent/python"));
    assert_eq!(record.stop, StopReason::Error);
    assert!(record.error.is_some());
    assert!(!record.goal_reached);
}

#[test]
fn an_offline_matrix_runs_in_order_and_writes_its_summary() {
    let dir = tempfile::tempdir().unwrap();
    let jobs = plan(
        &[Benchmark::MathProblem],
        &[ModelChoice::Offline],
        2,
        dir.path(),
    );
    let seen = Mutex::new(vec![]);
    let records = execute(jobs, &python().unwrap(), 2, &|r| {
        seen.lock().unwrap().push(r.repeat);
    });
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(records.iter().map(|r| r.repeat).collect::<Vec<_>>(), [1, 2]);
    for record in &records {
        assert!(record.goal_reached, "{record:?}");
        assert_eq!(
            (record.epochs, record.promotions, record.rejected),
            (3, 2, 1)
        );
        assert!(record.run_dir.join("ledger.json").exists());
    }
    let summary = Summary {
        rows: aggregate(&records),
        runs: records,
    };
    let (json, markdown) = write_summary(dir.path(), &summary, "# Loop benchmarks").unwrap();
    let back: Summary = serde_json::from_slice(&std::fs::read(json).unwrap()).unwrap();
    assert_eq!(back, summary);
    let text = std::fs::read_to_string(markdown).unwrap();
    assert!(text.starts_with("# Loop benchmarks\n\n| Benchmark |"));
    assert!(text.contains("| math_problem | offline | 2/2 | 3 | 0 | 15 digits | 0% |"));
}
