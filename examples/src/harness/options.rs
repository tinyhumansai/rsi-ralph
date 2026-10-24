//! The benchmark runner's command line.
//!
//! ```text
//! benchmarks [--benchmarks a,b] [--models m1,m2] [--repeats N] [--jobs N] [--out DIR]
//! ```
//!
//! # Example
//!
//! ```
//! use rsi_ralph_examples::harness::options::Options;
//! let options = Options::parse(["--repeats", "3", "--benchmarks", "math_problem"].map(String::from))?;
//! assert_eq!(options.repeats, 3);
//! assert_eq!(options.benchmarks.len(), 1);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use crate::benchmarks::Benchmark;
use std::error::Error;
use std::path::PathBuf;

/// How to invoke the runner, printed with a parse error.
pub const USAGE: &str = "usage: benchmarks [--benchmarks a,b] [--models m1,m2] [--repeats N] \
[--jobs N] [--out DIR]\nbenchmarks: curve_fit, graph_coloring, math_problem, upgrade_file";

/// Parsed runner settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Benchmarks to run, in the order given (all, by default).
    pub benchmarks: Vec<Benchmark>,
    /// `OpenRouter` model ids from `--models`, if given.
    pub models: Option<Vec<String>>,
    /// Runs per benchmark × model, at least 1.
    pub repeats: u32,
    /// Runs executed concurrently, at least 1.
    pub jobs: usize,
    /// Output root from `--out`, if given.
    pub out: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            benchmarks: Benchmark::ALL.to_vec(),
            models: None,
            repeats: 1,
            jobs: 1,
            out: None,
        }
    }
}

impl Options {
    /// Parses `args`, the arguments after the program name.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown flag, a flag without a value, an
    /// unknown benchmark, an empty list, or a count that is zero or not a
    /// number.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let value = match flag.as_str() {
                "--benchmarks" | "--models" | "--repeats" | "--jobs" | "--out" => {
                    args.next().ok_or_else(|| format!("{flag} needs a value"))?
                }
                _ => return Err(format!("unknown argument {flag}").into()),
            };
            match flag.as_str() {
                "--benchmarks" => {
                    options.benchmarks = list(&flag, &value)?
                        .into_iter()
                        .map(|name| {
                            Benchmark::from_name(&name)
                                .ok_or_else(|| format!("unknown benchmark {name}"))
                        })
                        .collect::<Result<_, _>>()?;
                }
                "--models" => options.models = Some(list(&flag, &value)?),
                "--repeats" => options.repeats = count(&flag, &value)?,
                "--jobs" => options.jobs = usize::try_from(count(&flag, &value)?)?,
                _ => options.out = Some(PathBuf::from(value)),
            }
        }
        Ok(options)
    }
}

/// A non-empty comma-separated list, entries trimmed.
fn list(flag: &str, value: &str) -> Result<Vec<String>, String> {
    let items: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if items.is_empty() {
        return Err(format!("{flag} needs at least one entry"));
    }
    Ok(items)
}

/// A count of at least 1.
fn count(flag: &str, value: &str) -> Result<u32, String> {
    match value.parse::<u32>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("{flag} needs a whole number of at least 1")),
    }
}
