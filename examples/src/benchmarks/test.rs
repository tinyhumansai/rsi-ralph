//! Benchmark names, offline stand-ins, the judge requirement, and one
//! offline run end to end.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::python;

#[test]
fn names_round_trip_and_unknown_names_are_none() {
    for benchmark in Benchmark::ALL {
        assert_eq!(Benchmark::from_name(benchmark.name()), Some(benchmark));
    }
    assert_eq!(Benchmark::from_name("hive_goal"), None);
}

#[test]
fn every_benchmark_has_a_canned_supervisor_and_judges_only_where_scored() {
    for benchmark in Benchmark::ALL {
        assert!(benchmark.offline_supervisor().remaining() >= 3, "{benchmark:?}");
        let judged = matches!(benchmark, Benchmark::MathProblem | Benchmark::UpgradeFile);
        assert_eq!(benchmark.offline_judge().is_some(), judged, "{benchmark:?}");
        assert_eq!(benchmark.offline_models().judge.is_some(), judged);
        let definition = benchmark.definition();
        assert!(definition.fixtures.iter().any(|(f, _)| *f == definition.editable));
        assert!(definition.fixtures.iter().any(|(f, _)| *f == definition.checker));
    }
    let models = Benchmark::CurveFit.offline_models();
    assert!(format!("{models:?}").contains("judge: false"));
}

#[test]
fn a_judged_benchmark_without_a_judge_fails_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let run = RunLocation {
        id: "r".into(),
        dir: dir.path().join("run"),
    };
    let models = Models {
        supervisor: Arc::new(Benchmark::MathProblem.offline_supervisor()),
        judge: None,
    };
    let error = Benchmark::MathProblem
        .run(&run, Path::new("python3"), models)
        .unwrap_err();
    assert_eq!(error.to_string(), "judge required");
    assert!(!run.dir.exists());
}

#[test]
fn metric_only_benchmarks_ignore_a_judge() {
    let judge: Arc<dyn ModelPort> = Arc::new(Benchmark::MathProblem.offline_supervisor());
    assert!(Benchmark::GraphColoring
        .scorer("coloring.py", Some(judge))
        .is_ok());
}

#[test]
fn the_math_problem_reaches_its_goal_offline() {
    let dir = tempfile::tempdir().unwrap();
    let run = RunLocation {
        id: "math".into(),
        dir: dir.path().join("math"),
    };
    let report = Benchmark::MathProblem
        .run(&run, &python().unwrap(), Benchmark::MathProblem.offline_models())
        .unwrap();
    assert_eq!(report.reason, tinyrsi::StopReason::GoalReached);
    assert_eq!((report.epochs, report.promotions, report.rejected), (3, 2, 1));
    assert!(run.dir.join("events.jsonl").exists());
    assert!(run.dir.join("workspace/check_pi.py").exists());
}
