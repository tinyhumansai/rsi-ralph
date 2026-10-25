//! Command-line parsing and the subcommands.
//!
//! Each subcommand writes human-readable lines to the given writer, so tests
//! drive them without a terminal. Exit status: 0 on success, 1 when a loop or
//! run fails, 2 for invalid input (a malformed loop folder or arguments).

use std::{
    collections::BTreeMap,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};

use crate::{
    Error, Result, doctor, init,
    keys::Keys,
    manifest::{self, Loop, Manifest},
    serve,
    store::{self, RunStatus, Store},
    subject::Subject,
};

mod run;
mod upgrades;

pub use upgrades::{AddArgs, OutcomeArgs, UpgradesCommand};

/// Run improvement loops defined as folders of files.
#[derive(Debug, Parser)]
#[command(name = "rsi-ralph", version, about)]
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
    /// Write a starter loop folder from a bundled template.
    Init {
        /// The folder to create; it must not exist or must be empty. The loop
        /// is named after it.
        #[arg(required_unless_present = "list")]
        r#loop: Option<PathBuf>,
        /// Template to start from.
        #[arg(long, default_value = "graph-coloring")]
        template: String,
        /// List the bundled templates instead of writing one.
        #[arg(long, conflicts_with = "loop")]
        list: bool,
    },
    /// Check that the programs and keys a run needs are available.
    Doctor,
    /// Print a shell completion script, such as `rsi-ralph completions zsh`.
    Completions {
        /// The shell to complete for.
        shell: clap_complete::Shell,
    },
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
        /// Extra step environment, `KEY=VALUE`, overriding `rsi.json`'s `env`;
        /// recorded in the run's copy of the loop. Repeatable.
        #[arg(long = "env", value_name = "KEY=VALUE")]
        env: Vec<String>,
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
        /// Extra step environment, `KEY=VALUE`, overriding `rsi.json`'s `env`.
        /// Repeatable.
        #[arg(long = "env", value_name = "KEY=VALUE")]
        env: Vec<String>,
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
    /// Read a loop's upgrade log, or record an upgrade or its outcome.
    Upgrades {
        /// What to do with the log.
        #[command(subcommand)]
        command: UpgradesCommand,
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
            | Error::NoRun(_)
            | Error::NotEmpty(_)
            | Error::UpgradeExists(_),
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
        Command::Init {
            r#loop,
            template,
            list: _,
        } => match r#loop {
            Some(dir) => init_loop(&dir, &template, out),
            _ => templates(out),
        },
        Command::Doctor => doctor::run(store.home(), lookup, &mut |line| say(out, line)),
        Command::Completions { shell } => {
            let mut command = <Cli as clap::CommandFactory>::command();
            clap_complete::generate(shell, &mut command, "rsi-ralph", out);
            Ok(())
        }
        Command::Check { r#loop } => check(&r#loop, lookup, out),
        Command::Runs { r#loop } => runs(&store, r#loop.as_deref(), out),
        Command::Show { run } => show(&store, &run, out),
        Command::Upgrades { command } => upgrades::execute(command, &store, store::now(), out),
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
            env,
        } => {
            let files = with_env(manifest::load_dir(&r#loop)?, &env)?;
            manifest::parse(&files)?;
            let server = if no_serve {
                None
            } else {
                Some(serve::start(store.clone(), local(port))?)
            };
            if let Some(server) = &server {
                say(out, &format!("ui: {}", server.url()))?;
            }
            run::run(&files, &r#loop, &store, run.as_deref(), epochs, lookup, out)
        }
        Command::Score {
            r#loop,
            label,
            checkout,
            env,
        } => {
            let parsed = manifest::parse(&with_env(manifest::load_dir(&r#loop)?, &env)?)?;
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
    say(out, &check_summary(&parsed.manifest))
}

/// The one-line summary `check` prints for a valid loop.
///
/// It names how many seeds of a scorer run at once only when that is more
/// than one, so the default line is unchanged.
fn check_summary(m: &Manifest) -> String {
    let parallel = if m.score.parallel > 1 {
        format!(", {} seeds at once", m.score.parallel)
    } else {
        String::new()
    };
    format!(
        "ok: {} ({} subject, {} scorers, decide {} by {}{parallel}, hive lead {})",
        m.name,
        wire(&m.subject.kind),
        m.score.scorers.len(),
        m.score.decide.on,
        wire(&m.score.decide.test),
        m.hive.lead
    )
}

fn templates(out: &mut dyn Write) -> Result<()> {
    for t in init::TEMPLATES {
        say(out, &format!("{:<16} {}", t.name, t.about))?;
    }
    Ok(())
}

fn init_loop(dir: &Path, template: &str, out: &mut dyn Write) -> Result<()> {
    let name = init::write(init::template(template)?, dir)?;
    say(out, &format!("created {name} in {}", dir.display()))?;
    say(out, "next:")?;
    say(out, &format!("  rsi-ralph check {}", dir.display()))?;
    say(out, &format!("  rsi-ralph run   {}", dir.display()))
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
    for warning in &r.warnings {
        say(out, &format!("warning   {warning}"))?;
    }
    say(out, &format!("folder    {}", dir.path.display()))
}

/// The loop folder's files with `overrides` (`KEY=VALUE`) merged into
/// `rsi.json`'s `env`, so a run's frozen copy records what its steps saw.
/// The result still goes through [`manifest::parse`], which validates names.
fn with_env(
    mut files: BTreeMap<String, String>,
    overrides: &[String],
) -> Result<BTreeMap<String, String>> {
    if overrides.is_empty() {
        return Ok(files);
    }
    let source = files
        .get(manifest::MANIFEST)
        .ok_or_else(|| Error::MissingFile {
            path: manifest::MANIFEST.into(),
        })?;
    let mut value: serde_json::Value = serde_json::from_str(source).map_err(|e| Error::Json {
        message: e.to_string(),
    })?;
    let env = value
        .as_object_mut()
        .ok_or_else(|| Error::field("rsi.json", "must be a JSON object"))?
        .entry("env")
        .or_insert_with(|| serde_json::json!({}));
    let env = env
        .as_object_mut()
        .ok_or_else(|| Error::field("env", "must be an object"))?;
    for pair in overrides {
        let (key, val) = pair
            .split_once('=')
            .filter(|(key, _)| !key.is_empty())
            .ok_or_else(|| Error::field(format!("--env {pair}"), "expected KEY=VALUE"))?;
        env.insert(key.to_owned(), serde_json::Value::String(val.to_owned()));
    }
    let text = serde_json::to_string_pretty(&value).map_err(|e| Error::Json {
        message: e.to_string(),
    })?;
    files.insert(manifest::MANIFEST.to_owned(), text + "\n");
    Ok(files)
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
