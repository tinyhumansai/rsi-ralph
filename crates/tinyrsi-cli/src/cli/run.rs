//! `tinyrsi run` and `tinyrsi score`: wiring keys, the spend gateway, judges,
//! the hive and the retreat into the engine.

use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use tinyrsi_gateway::{Gateway, GatewayConfig, Price};
use tinyrsi_model::OpenRouterModel;
use tinyrsi_score::{JudgeLimits, LlmJudge};

use super::say;
use crate::{
    Error, Result,
    engine::{Engine, Meter},
    hive::Hive,
    keys::Keys,
    manifest::{self, Loop},
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
}

/// `PATH` and `HOME` from the caller, for steps.
fn inherited(lookup: &dyn Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    ["PATH", "HOME"]
        .into_iter()
        .filter_map(|k| lookup(k).map(|v| (k.to_owned(), v)))
        .collect()
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
    Gateway::start(config).map_err(|e| Error::Proposer(format!("gateway: {e}")))
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

/// `tinyrsi run`.
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
        inherited: inherited(lookup),
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
    let spend = gateway.spend();
    say(
        out,
        &format!(
            "spend: ${:.2} this run, ${:.2} in total",
            spend.run_usd, spend.total_usd
        ),
    )
}

/// `tinyrsi score`.
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
        inherited: inherited(lookup),
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
