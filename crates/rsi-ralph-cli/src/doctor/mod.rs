//! `rsi-ralph doctor`: check the machine before a first run.
//!
//! A loop shells out to `git` and `diff`, the bundled templates score with
//! `python3`, and every run needs an OpenRouter key. `doctor` reports each of
//! those from the environment it is given, so tests pass a map instead of
//! the process environment. It never prints a key's value.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::{Error, Result};

/// A program a loop may start, and whether a run cannot work without it.
#[derive(Debug, Clone, Copy)]
pub struct Tool {
    /// The program name, without an extension.
    pub name: &'static str,
    /// What uses it, shown next to its status.
    pub used_for: &'static str,
    /// Whether its absence makes `doctor` fail.
    pub required: bool,
}

/// The programs `doctor` looks for.
pub const TOOLS: &[Tool] = &[
    Tool {
        name: "git",
        used_for: "git subjects",
        required: true,
    },
    Tool {
        name: "diff",
        used_for: "dir subject diffs",
        required: true,
    },
    Tool {
        name: "python3",
        used_for: "the bundled template's scorer",
        required: false,
    },
];

/// The key every run needs, by name.
pub const MODEL_KEY: &str = "OPENROUTER_API_KEY";

/// Write one line per check to `say` and fail when a required tool is missing.
///
/// `home` is the run store; `lookup` reads the environment.
///
/// # Errors
/// [`Error::Doctor`] naming every missing required tool, or whatever `say`
/// returns.
pub fn run(
    home: &Path,
    lookup: &dyn Fn(&str) -> Option<String>,
    say: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<()> {
    say(&format!(
        "rsi-ralph {} ({}-{})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    ))?;
    let path = lookup("PATH").map(OsString::from);
    let mut missing = Vec::new();
    for tool in TOOLS {
        match find(tool.name, path.as_deref()) {
            Some(found) => say(&format!("ok      {:<8} {}", tool.name, found.display()))?,
            None => {
                let mark = if tool.required { "missing" } else { "absent " };
                say(&format!("{mark} {:<8} needed for {}", tool.name, tool.used_for))?;
                if tool.required {
                    missing.push(tool.name.to_owned());
                }
            }
        }
    }
    if lookup(MODEL_KEY).is_some_and(|v| !v.is_empty()) {
        say(&format!("ok      {MODEL_KEY} is set"))?;
    } else {
        say(&format!(
            "absent  {MODEL_KEY} (export it or put it in <loop>/.env before `run`)"
        ))?;
    }
    let state = if home.is_dir() {
        "exists"
    } else {
        "created on first run"
    };
    say(&format!("store   {} ({state})", home.display()))?;
    if missing.is_empty() {
        Ok(())
    } else {
        Err(Error::Doctor { missing })
    }
}

/// The first file named `name` in the `PATH` directories, trying `.exe` on
/// Windows.
fn find(name: &str, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let names: &[String] = &if cfg!(windows) {
        vec![format!("{name}.exe"), name.to_owned()]
    } else {
        vec![name.to_owned()]
    };
    std::env::split_paths(path?).find_map(|dir| {
        names
            .iter()
            .map(|n| dir.join(n))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(test)]
mod test;
