//! Command-line parsing.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::options::{Options, USAGE};
use crate::benchmarks::Benchmark;
use std::path::PathBuf;

fn parse(args: &[&str]) -> Result<Options, String> {
    Options::parse(args.iter().map(|s| (*s).to_owned())).map_err(|e| e.to_string())
}

#[test]
fn no_arguments_run_every_benchmark_once() {
    let options = parse(&[]).unwrap();
    assert_eq!(options, Options::default());
    assert_eq!(options.benchmarks, Benchmark::ALL);
    assert_eq!((options.repeats, options.jobs), (1, 1));
    assert_eq!((options.models, options.out), (None, None));
    assert!(USAGE.contains("--repeats"));
}

#[test]
fn every_flag_is_read() {
    let options = parse(&[
        "--benchmarks",
        "upgrade_file, curve_fit",
        "--models",
        "a/b,c/d",
        "--repeats",
        "5",
        "--jobs",
        "2",
        "--out",
        "docs/rsi/results/x",
    ])
    .unwrap();
    assert_eq!(
        options.benchmarks,
        [Benchmark::UpgradeFile, Benchmark::CurveFit]
    );
    assert_eq!(options.models.unwrap(), ["a/b", "c/d"]);
    assert_eq!((options.repeats, options.jobs), (5, 2));
    assert_eq!(options.out, Some(PathBuf::from("docs/rsi/results/x")));
}

#[test]
fn bad_arguments_are_rejected_with_a_reason() {
    assert_eq!(parse(&["--fast"]).unwrap_err(), "unknown argument --fast");
    assert_eq!(
        parse(&["--repeats"]).unwrap_err(),
        "--repeats needs a value"
    );
    assert_eq!(
        parse(&["--repeats", "0"]).unwrap_err(),
        "--repeats needs a whole number of at least 1"
    );
    assert_eq!(
        parse(&["--jobs", "many"]).unwrap_err(),
        "--jobs needs a whole number of at least 1"
    );
    assert_eq!(
        parse(&["--benchmarks", "hive_goal"]).unwrap_err(),
        "unknown benchmark hive_goal"
    );
    assert_eq!(
        parse(&["--models", " , "]).unwrap_err(),
        "--models needs at least one entry"
    );
}
