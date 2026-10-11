//! The script-loop benchmarks, defined once for the example binaries and the
//! benchmark runner.
//!
//! Each [`Benchmark`] bundles a subject's Python fixtures (embedded with
//! `include_str!` from where they already live), its brief, metric, seeds,
//! scorer, loop policy, and the canned supervisor and scripted judge it runs
//! on offline. [`Benchmark::run`] materializes one run of it in a
//! [`RunLocation`]. Benchmarks and their policies are listed in
//! `docs/specs/loop-benchmarks.md`.
//!
//! # Example
//!
//! ```
//! use tinyrsi_examples::benchmarks::Benchmark;
//! let names: Vec<_> = Benchmark::ALL.iter().map(|b| b.name()).collect();
//! assert_eq!(names, ["curve_fit", "graph_coloring", "math_problem", "upgrade_file"]);
//! assert_eq!(Benchmark::from_name("graph_coloring"), Some(Benchmark::GraphColoring));
//! assert!(Benchmark::GraphColoring.offline_judge().is_none());
//! ```
mod curve_fit;
mod graph_coloring;
mod math_problem;
mod upgrade_file;

use crate::{Grader, Replies};
use std::{error::Error, path::Path, sync::Arc};
use tinyrsi::{
    Composite, JsonlTrace, JudgeLimits, LlmJudge, LoopPolicy, LoopReport, MetricDirection,
    MetricScorer, MetricSpec, ModelPort, MonotonicClock, Projected, RunLocation, Scorer,
    ScriptCandidate, ScriptCommand, ScriptSubject, ScriptSupervisor, SupervisorLimits,
    run_script_loop_with,
};

/// One script-loop benchmark: a subject, its goal and its offline stand-ins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Benchmark {
    /// Fit a noisy damped sinusoid (`model.py`, RMSE down to 0.06).
    CurveFit,
    /// Colour graphs with a hidden 6-colouring (`coloring.py`, down to 6).
    GraphColoring,
    /// Compute pi (`pi.py`, up to 12 correct digits), with a judge.
    MathProblem,
    /// Bring `slug.py` to a written standard (0 violations), with a judge.
    UpgradeFile,
}

/// The model ports one benchmark run uses.
pub struct Models {
    /// Answers the script supervisor's proposal requests.
    pub supervisor: Arc<dyn ModelPort>,
    /// Grades sources for a benchmark whose scorer has a judge; ignored by
    /// metric-only benchmarks.
    pub judge: Option<Arc<dyn ModelPort>>,
}

impl std::fmt::Debug for Models {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Models")
            .field("judge", &self.judge.is_some())
            .finish_non_exhaustive()
    }
}

/// What a benchmark module supplies to build its subject.
struct Definition {
    /// Subject name in the trace and ledger.
    name: &'static str,
    /// Files written into the workspace: `(file name, contents)`.
    fixtures: &'static [(&'static str, &'static str)],
    /// The one editable file.
    editable: &'static str,
    /// The fixed checker the interpreter runs.
    checker: &'static str,
    /// The supervisor's brief.
    brief: String,
    /// Metric and goal.
    metric: MetricSpec,
    /// Seeds every candidate runs on.
    seeds: Vec<u64>,
    /// Per-seed timeout.
    timeout_ms: u64,
    /// The loop policy.
    policy: LoopPolicy,
}

impl Benchmark {
    /// Every benchmark, in the order the runner reports them.
    pub const ALL: [Self; 4] = [
        Self::CurveFit,
        Self::GraphColoring,
        Self::MathProblem,
        Self::UpgradeFile,
    ];

    /// The benchmark's command-line and directory name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::CurveFit => "curve_fit",
            Self::GraphColoring => "graph_coloring",
            Self::MathProblem => "math_problem",
            Self::UpgradeFile => "upgrade_file",
        }
    }

    /// The benchmark called `name`, if any.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.name() == name)
    }

    /// A fresh canned supervisor: the benchmark's prepared rewrites, in order.
    #[must_use]
    pub fn offline_supervisor(self) -> Replies {
        match self {
            Self::CurveFit => curve_fit::canned(),
            Self::GraphColoring => graph_coloring::canned(),
            Self::MathProblem => math_problem::canned(),
            Self::UpgradeFile => upgrade_file::canned(),
        }
    }

    /// The scripted judge, for a benchmark whose scorer has one.
    #[must_use]
    pub fn offline_judge(self) -> Option<Grader> {
        match self {
            Self::CurveFit | Self::GraphColoring => None,
            Self::MathProblem => Some(math_problem::grader()),
            Self::UpgradeFile => Some(upgrade_file::grader()),
        }
    }

    /// The offline model ports: the canned supervisor and, when the
    /// benchmark has one, the scripted judge.
    #[must_use]
    pub fn offline_models(self) -> Models {
        Models {
            supervisor: Arc::new(self.offline_supervisor()),
            judge: self
                .offline_judge()
                .map(|g| Arc::new(g) as Arc<dyn ModelPort>),
        }
    }

    /// Runs the benchmark once in `run`: writes its fixtures to
    /// `run.dir/workspace/`, then runs the script loop with `python`, the
    /// given model ports, and a trace at `run.dir/events.jsonl`.
    ///
    /// **Safety:** proposals are code that runs locally; see the script loop
    /// guide.
    ///
    /// # Errors
    ///
    /// Returns an error when the benchmark has a judge and `models.judge` is
    /// `None` (`judge required`), when the workspace or trace cannot be
    /// written, or when the loop itself fails (see
    /// [`tinyrsi::run_script_loop_with`]).
    pub fn run(
        self,
        run: &RunLocation,
        python: &Path,
        models: Models,
    ) -> Result<LoopReport, Box<dyn Error>> {
        let definition = self.definition();
        let scorer = self.scorer(definition.editable, models.judge)?;
        let workspace = run.dir.join("workspace");
        std::fs::create_dir_all(&workspace)?;
        for (file, contents) in definition.fixtures {
            std::fs::write(workspace.join(file), contents)?;
        }
        let subject = ScriptSubject {
            name: definition.name.into(),
            brief: definition.brief,
            workspace,
            editable: vec![definition.editable.into()],
            command: ScriptCommand {
                program: python.to_path_buf(),
                args: vec![definition.checker.into()],
            },
            metric: definition.metric,
            seeds: definition.seeds,
            timeout_ms: definition.timeout_ms,
            output_cap: 65_536,
            env: vec![("PYTHONDONTWRITEBYTECODE".into(), "1".into())],
        };
        let supervisor = ScriptSupervisor::new(models.supervisor, SupervisorLimits::default())?;
        let mut trace = JsonlTrace::create(run.dir.join("events.jsonl"), MonotonicClock::new())?;
        Ok(run_script_loop_with(
            &subject,
            &supervisor,
            &*scorer,
            &definition.policy,
            run,
            &mut trace,
        )?)
    }

    fn definition(self) -> Definition {
        match self {
            Self::CurveFit => curve_fit::definition(),
            Self::GraphColoring => graph_coloring::definition(),
            Self::MathProblem => math_problem::definition(),
            Self::UpgradeFile => upgrade_file::definition(),
        }
    }

    /// The metric alone, or the metric blended with a judge reading the
    /// editable file.
    fn scorer(
        self,
        editable: &'static str,
        judge: Option<Arc<dyn ModelPort>>,
    ) -> Result<Box<dyn Scorer<ScriptCandidate>>, Box<dyn Error>> {
        let direction = self.definition().metric.direction;
        let metric = Projected::new(
            |c: &ScriptCandidate| c.metrics.as_slice(),
            MetricScorer::new(direction),
        );
        // The weights each example has always used, written out so they sum
        // exactly as before rather than through `1.0 - w`.
        let (metric_weight, judge_weight) = match self {
            Self::CurveFit | Self::GraphColoring => return Ok(Box::new(metric)),
            Self::MathProblem => (0.7, 0.3),
            Self::UpgradeFile => (0.6, 0.4),
        };
        let judge = LlmJudge::new(judge.ok_or("judge required")?, 1, JudgeLimits::default())?;
        let source = Projected::new(
            move |c: &ScriptCandidate| {
                c.sources
                    .get(Path::new(editable))
                    .map_or("", String::as_str)
            },
            judge,
        );
        Ok(Box::new(
            Composite::new()
                .with(metric_weight, metric)
                .with(judge_weight, source),
        ))
    }
}

/// A minimizing or maximizing metric with a target.
fn metric(name: &str, direction: MetricDirection, target: f64) -> MetricSpec {
    MetricSpec {
        name: name.into(),
        direction,
        target: Some(target),
    }
}

#[cfg(test)]
mod test;
