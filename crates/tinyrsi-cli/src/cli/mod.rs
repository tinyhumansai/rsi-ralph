//! Command-line parsing and the subcommands.
//!
//! Each subcommand writes human-readable lines to the given writer, so tests
//! drive them without a terminal. Exit status: 0 on success, 1 when a loop or
//! run fails, 2 for invalid input (a malformed loop folder or arguments).

use std::{
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};

use crate::{
    Error, Result,
    keys::Keys,
    manifest::{self, Loop},
    serve,
    store::{RunStatus, Store},
    subject::Subject,
};

mod run;

/// Run improvement loops defined as folders of files.
#[derive(Debug, Parser)]
#[command(name = "tinyrsi", version, about)]
pub struct Cli {
    /// Run store folder; defaults to `$TINYRSI_HOME`, then `~/.tinyrsi`.
    #[arg(long, global = true)]
    pub home: Option<PathBuf>,
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// The subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate a loop folder and report missing keys.
    Check {
        /// The loop folder.
        r#loop: PathBuf,
    },
    /// Run a loop, serving the UI alongside it.
    Run {
        /// The loop folder.
        r#loop: PathBuf,
        /// Run id; defaults to the current UTC time.
        #[arg(long)]
        run: Option<String>,
        /// Epoch limit overriding `rsi.json`.
        #[arg(long)]
        epochs: Option<u32>,
        /// UI port.
        #[arg(long, default_value_t = 8787)]
        port: u16,
        /// Do not serve the UI.
        #[arg(long)]
        no_serve: bool,
    },
    /// Score one checkout with the loop's scorers, outside any run.
    Score {
        /// The loop folder.
        r#loop: PathBuf,
        /// Label recorded with the scores.
        #[arg(long, default_value = "manual")]
        label: String,
        /// Checkout to score; defaults to the subject's incumbent.
        #[arg(long)]
        checkout: Option<PathBuf>,
    },
    /// List stored runs, newest first.
    Runs {
        /// Only this loop's runs.
        #[arg(long)]
        r#loop: Option<String>,
    },
    /// Summarize one stored run.
    Show {
        /// `<loop>/<run-id>`.
        run: String,
    },
    /// Serve the UI over the run store.
    Serve {
        /// Port.
        #[arg(long, default_value_t = 8787)]
        port: u16,
    },
}

/// Exit status for a result.
#[must_use]
pub fn exit_code(result: &Result<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(
            Error::MissingFile { .. }
            | Error::Json { .. }
            | Error::Schema { .. }
            | Error::Field { .. }
            | Error::Path { .. }
            | Error::Reference { .. }
            | Error::Hive(_)
            | Error::MissingKeys { .. }
            | Error::RunExists(_)
            | Error::NoRun(_),
        ) => 2,
        Err(_) => 1,
    }
}

/// Run a parsed command.
///
/// `lookup` reads the process environment; tests pass a map.
///
/// # Errors
/// Returns the command's error; see [`exit_code`] for how it maps to a status.
pub fn execute(
    cli: Cli,
    lookup: &dyn Fn(&str) -> Option<String>,
    out: &mut dyn Write,
) -> Result<()> {
    let store = Store::new(cli.home.unwrap_or_else(|| Store::default_home(lookup)));
    match cli.command {
        Command::Check { r#loop } => check(&r#loop, lookup, out),
        Command::Runs { r#loop } => runs(&store, r#loop.as_deref(), out),
        Command::Show { run } => show(&store, &run, out),
        Command::Serve { port } => {
            let server = serve::start(store, local(port))?;
            say(out, &format!("serving {}", server.url()))?;
            loop {
                std::thread::park();
            }
        }
        Command::Run {
            r#loop,
            run,
            epochs,
            port,
            no_serve,
        } => {
            let parsed = load(&r#loop)?;
            let server = if no_serve {
                None
            } else {
                Some(serve::start(store.clone(), local(port))?)
            };
            if let Some(server) = &server {
                say(out, &format!("ui: {}", server.url()))?;
            }
            run::run(
                &parsed,
                &r#loop,
                &store,
                run.as_deref(),
                epochs,
                lookup,
                out,
            )
        }
        Command::Score {
            r#loop,
            label,
            checkout,
        } => {
            let parsed = load(&r#loop)?;
            run::score(
                &parsed,
                &r#loop,
                &store,
                &label,
                checkout.as_deref(),
                lookup,
                out,
            )
        }
    }
}

/// The wire name of a serializable enum, such as `max_epochs`.
fn wire(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value)
        .unwrap_or_default()
        .trim_matches('"')
        .to_owned()
}

fn score_text(score: Option<f64>) -> String {
    score.map_or_else(|| "-".to_owned(), |s| format!("{s:.4}"))
}

fn check(dir: &Path, lookup: &dyn Fn(&str) -> Option<String>, out: &mut dyn Write) -> Result<()> {
    let parsed = load(dir)?;
    Keys::resolve(&parsed, dir, lookup)?;
    Subject::from_spec(&parsed.manifest.subject, dir).verify()?;
    let m = &parsed.manifest;
    say(
        out,
        &format!(
            "ok: {} ({} subject, {} scorers, decide {} by {}, hive lead {})",
            m.name,
            wire(&m.subject.kind),
            m.score.scorers.len(),
            m.score.decide.on,
            wire(&m.score.decide.test),
            m.hive.lead
        ),
    )
}

fn runs(store: &Store, loop_name: Option<&str>, out: &mut dyn Write) -> Result<()> {
    for r in store.list_runs(loop_name)? {
        say(
            out,
            &format!(
                "{}/{}\t{}\tepochs {}\tscore {}\t${:.2}",
                r.r#loop,
                r.id,
                status(r.status),
                r.epochs,
                score_text(r.incumbent_score),
                r.spend_usd
            ),
        )?;
    }
    Ok(())
}

fn show(store: &Store, run: &str, out: &mut dyn Write) -> Result<()> {
    let (dir, r) = store.find_run(run)?;
    say(out, &format!("run       {}/{}", r.r#loop, r.id))?;
    say(out, &format!("status    {}", status(r.status)))?;
    if let Some(reason) = r.stop_reason {
        say(out, &format!("stopped   {}", wire(&reason)))?;
    }
    say(out, &format!("epochs    {}", r.epochs))?;
    say(
        out,
        &format!(
            "incumbent {} ({})",
            r.incumbent,
            score_text(r.incumbent_score)
        ),
    )?;
    say(out, &format!("spend     ${:.2}", r.spend_usd))?;
    if let Some(error) = r.error {
        say(out, &format!("error     {error}"))?;
    }
    say(out, &format!("folder    {}", dir.path.display()))
}

fn load(dir: &Path) -> Result<Loop> {
    manifest::parse(&manifest::load_dir(dir)?)
}

fn local(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn status(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "running",
        RunStatus::Finished => "finished",
        RunStatus::Failed => "failed",
        RunStatus::Interrupted => "interrupted",
    }
}

fn say(out: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(out, "{line}").map_err(|e| Error::io("write", "stdout", e))
}

#[cfg(test)]
mod test;
