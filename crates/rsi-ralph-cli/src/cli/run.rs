//! `rsi-ralph run` and `rsi-ralph score`: wiring keys, the spend gateway, judges,
//! the hive and the retreat into the engine.

use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use rsi_ralph_gateway::{CallMark, Gateway, GatewayConfig, Limits, Price, UpstreamCalls};
use rsi_ralph_model::OpenRouterModel;
use rsi_ralph_score::{JudgeLimits, LlmJudge};
use rsi_ralph_trace::StopReason;

use super::say;
use crate::{
    Error, Result,
    engine::{Engine, Meter},
    hive::Hive,
    keys::Keys,
    manifest::{self, Budget, Loop},
    retreat::Retreat,
    score::{self, Judges},
    store::{self, RunDir, Store},
    subject::{Checkout, Subject},
};

/// Spend read from the in-process gateway.
struct GatewayMeter<'a>(&'a Gateway);

impl Meter for GatewayMeter<'_> {
    fn spent_usd(&self) -> f64 {
        self.0.spend().run_usd
    }

    fn remaining_usd(&self) -> Option<f64> {
        let remaining = self.0.spend().remaining_usd();
        remaining.is_finite().then_some(remaining)
    }

    fn call_mark(&self) -> CallMark {
        self.0.call_mark()
    }

    fn upstream_calls_since(&self, mark: CallMark) -> UpstreamCalls {
        self.0.upstream_calls_since(mark)
    }
}

/// The caller's variables that steps inherit: `PATH` and `HOME`, plus those
/// Windows programs need to start (Python aborts without `SYSTEMROOT`) or to
/// find the user's folders. Unset names are skipped, so elsewhere this is
/// `PATH` and `HOME`.
const INHERITED: &[&str] = &[
    "PATH",
    "HOME",
    "SYSTEMROOT",
    "WINDIR",
    "COMSPEC",
    "PATHEXT",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
];

/// [`INHERITED`] from the caller, for steps.
pub(super) fn inherited(lookup: &dyn Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    INHERITED
        .iter()
        .copied()
        .filter_map(|k| lookup(k).map(|v| (k.to_owned(), v)))
        .collect()
}

/// What steps inherit: [`inherited`] plus the loop's plain step keys, each
/// under its own variable name. Their values are already among the secrets
/// scrubbed from anything stored.
fn step_env(
    parsed: &Loop,
    keys: &Keys,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    let mut env = inherited(lookup);
    for name in parsed.step_key_envs() {
        if let Some(secret) = keys.get(name) {
            env.push((name.to_owned(), secret.expose().to_owned()));
        }
    }
    env
}

/// Start the spend gateway for one run folder.
///
/// `TINYRSI_UPSTREAM` replaces `OpenRouter` with another OpenAI-compatible base
/// URL, such as a local model server.
fn gateway(
    parsed: &Loop,
    keys: &Keys,
    store: &Store,
    run: &RunDir,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<Gateway> {
    let key = keys
        .get(parsed.openrouter_env())
        .ok_or_else(|| Error::MissingKeys {
            names: vec![parsed.openrouter_env().to_owned()],
        })?;
    let budget = &parsed.manifest.budget;
    let mut config = GatewayConfig::new(
        key.expose().to_owned(),
        run.path.clone(),
        store.spend_ledger(),
        budget.run_usd.unwrap_or(f64::MAX),
        budget.total_usd.unwrap_or(f64::MAX),
    );
    if let Some(upstream) = lookup("TINYRSI_UPSTREAM").filter(|u| !u.is_empty()) {
        config.upstream = upstream;
    }
    for (model, price) in &budget.prices {
        config.prices.insert(
            model.clone(),
            Price {
                prompt: price.prompt_per_mtok / 1e6,
                completion: price.completion_per_mtok / 1e6,
                provider: price.provider.clone(),
            },
        );
    }
    config.limits = gateway_limits(budget);
    Gateway::start(config).map_err(|e| Error::Proposer(format!("gateway: {e}")))
}

/// The gateway's completion limits under `budget`: its `max_tokens` replaces
/// the stock cap, and lowers the default to match when it is smaller.
pub(super) fn gateway_limits(budget: &Budget) -> Limits {
    let mut limits = Limits::default();
    if let Some(cap) = budget.max_tokens {
        limits.max_tokens_cap = cap;
        limits.default_max_tokens = limits.default_max_tokens.min(cap);
    }
    limits
}

/// One `LlmJudge` per judge scorer, calling through the gateway.
fn judges(parsed: &Loop, gateway: &Gateway) -> Result<Judges> {
    let mut judges = Judges::new();
    for scorer in &parsed.manifest.score.scorers {
        if scorer.judge.is_none() {
            continue;
        }
        let port = OpenRouterModel::via(
            parsed.judge_model().to_owned(),
            &gateway.tagged_url("judge", &scorer.id),
        )
        .map_err(|e| Error::Proposer(format!("judge {}: {e}", scorer.id)))?;
        let judge = LlmJudge::new(Arc::new(port), scorer.raters, JudgeLimits::default())?;
        judges.insert(scorer.id.clone(), Box::new(judge));
    }
    Ok(judges)
}

/// `rsi-ralph run`.
pub(super) fn run(
    files: &BTreeMap<String, String>,
    loop_dir: &Path,
    store: &Store,
    run_id: Option<&str>,
    epochs: Option<u32>,
    lookup: &dyn Fn(&str) -> Option<String>,
    out: &mut dyn Write,
) -> Result<()> {
    let parsed = &manifest::parse(files)?;
    let keys = Keys::resolve(parsed, loop_dir, lookup)?;
    Subject::from_spec(&parsed.manifest.subject, loop_dir).verify()?;
    let run = store.create_run(&parsed.manifest.name, run_id, files)?;
    say(
        out,
        &format!(
            "run: {}/{} in {}",
            run.loop_name,
            run.id,
            run.path.display()
        ),
    )?;
    let gateway = gateway(parsed, &keys, store, &run, lookup)?;
    let meter = GatewayMeter(&gateway);
    let mut hive = Hive::new(parsed, &keys, &gateway.base_url())?;
    let mut retreat = Retreat::new(parsed, store, &gateway)?;
    let engine = Engine {
        parsed,
        loop_dir: absolute(loop_dir),
        store,
        run,
        proposer: &mut hive,
        ideator: retreat.as_mut().map(|r| r as _),
        judges: judges(parsed, &gateway)?,
        meter: Some(&meter),
        model_url: Some(gateway.base_url()),
        inherited: step_env(parsed, &keys, lookup),
        secrets: keys.values().map(str::to_owned).collect(),
        epochs,
    };
    let summary = engine.run()?;
    say(
        out,
        &format!(
            "stopped: {} after {} promotions; incumbent {} scores {} ({})",
            serde_json::to_string(&summary.reason)
                .unwrap_or_default()
                .trim_matches('"'),
            summary.promotions,
            summary.incumbent,
            summary
                .score
                .map_or_else(|| "-".to_owned(), |s| format!("{s:.4}")),
            summary.checkout.display()
        ),
    )?;
    for warning in &summary.warnings {
        say(out, &format!("warning: {warning}"))?;
    }
    let spend = gateway.spend();
    say(
        out,
        &format!(
            "spend: ${:.2} this run, ${:.2} in total",
            spend.run_usd, spend.total_usd
        ),
    )?;
    // The run is stored and reported; the command still fails, so scripts
    // see a provider that refused everything as the failure it is.
    if summary.reason == StopReason::ProviderRefused {
        return Err(Error::ProviderRefused(summary.error.unwrap_or_default()));
    }
    Ok(())
}

/// `rsi-ralph score`.
pub(super) fn score(
    parsed: &Loop,
    loop_dir: &Path,
    store: &Store,
    label: &str,
    checkout: Option<&Path>,
    lookup: &dyn Fn(&str) -> Option<String>,
    out: &mut dyn Write,
) -> Result<()> {
    let keys = Keys::resolve(parsed, loop_dir, lookup)?;
    let loop_dir = absolute(loop_dir);
    let run = store.create_run(
        &parsed.manifest.name,
        Some(&format!(
            "score-{label}-{}",
            store::timestamp_id(store::now())
        )),
        &BTreeMap::new(),
    )?;
    let gateway = gateway(parsed, &keys, store, &run, lookup)?;
    let subject = Subject::from_spec(&parsed.manifest.subject, &loop_dir);
    let target = match checkout {
        Some(path) => Checkout {
            label: label.to_owned(),
            path: absolute(path),
        },
        None => subject.incumbent(&run.path)?,
    };
    let ctx = crate::step::StepContext {
        loop_dir: loop_dir.clone(),
        run_dir: run.path.clone(),
        incumbent: target.path.clone(),
        epoch: 0,
        model_url: Some(gateway.base_url()),
        env: parsed.manifest.env.clone(),
        inherited: step_env(parsed, &keys, lookup),
    };
    let card = score::score_checkout(
        &ctx,
        &parsed.manifest.score,
        &target,
        &run.path.join("steps"),
        &Judges::new(),
        &parsed.rubrics,
        None,
    )?;
    store::write_json(&run.path.join("scorecard.json"), &card)?;
    for (id, runs) in &card.runs {
        for seed in runs {
            let value = seed.result.as_ref().map_or_else(
                || format!("failed: {}", seed.reason.as_deref().unwrap_or("?")),
                |r| format!("{:.4}", r.score),
            );
            say(out, &format!("{id}\tseed {}\t{value}", seed.seed))?;
        }
    }
    say(
        out,
        &format!(
            "{} = {} (spend ${:.2}; details in {})",
            parsed.manifest.score.decide.on,
            card.headline
                .map_or_else(|| "-".to_owned(), |s| format!("{s:.4}")),
            gateway.spend().run_usd,
            run.path.display()
        ),
    )
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}
