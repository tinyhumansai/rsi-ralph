//! Bind private numeric tasks to immutable golden checkers without exporting oracles.
mod calibration;
mod types;
use rsi_ralph_host::{Error, HostVerifier, PrivateSuites, PrivateTask, Result};
use std::collections::{BTreeMap, BTreeSet};

const FAMILIES: [&str; 4] = ["assignment", "euclid", "harshad", "idempotents"];
const STAGES: [&str; 4] = ["anchor", "confirmation", "screen", "selection"];

/// Read one host-private bank and print only its public suite identities.
pub(super) fn cli(args: &[String]) -> std::result::Result<(), Box<dyn std::error::Error>> {
    if args.len() != 2 {
        return Err("usage: private_benchmark <private-bank.json>".into());
    }
    let bytes = std::fs::read(&args[1])?;
    let suites = freeze(&bytes)?;
    let samples = calibration::verify(&bytes)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "suite_fingerprints": suites.fingerprints(), "verified_calibration_samples": samples
        }))?
    );
    Ok(())
}

/// Decode and freeze an exact four-stage numerical bank, returning generic errors.
///
/// No private input is included in a parse error. Golden checks verify final
/// answers; they do not certify a submitted algorithm or formal proof.
pub(super) fn freeze(bytes: &[u8]) -> Result<PrivateSuites> {
    let bank: types::Bank = serde_json::from_slice(bytes).map_err(|_| Error::InvalidEvaluation)?;
    if bank.version != 1
        || bank.provenance != "agent-authored; two independent integer oracles"
        || bank.private_seed.len() != 64
        || !bank.private_seed.bytes().all(|b| b.is_ascii_hexdigit())
        || bank.stages.keys().map(String::as_str).collect::<Vec<_>>() != STAGES
    {
        return Err(Error::InvalidEvaluation);
    }
    let mut stages = BTreeMap::new();
    let mut prompts = BTreeSet::new();
    let mut seeds = BTreeSet::new();
    let mut stage_len = None;
    for (stage, rows) in bank.stages {
        if rows.is_empty() || stage_len.is_some_and(|n| n != rows.len()) {
            return Err(Error::InvalidEvaluation);
        }
        stage_len = Some(rows.len());
        let mut families = BTreeMap::new();
        let mut tasks = Vec::new();
        for row in rows {
            let number = row
                .expected
                .strip_suffix('\n')
                .ok_or(Error::InvalidEvaluation)?;
            let parsed: i128 = number.parse().map_err(|_| Error::InvalidEvaluation)?;
            if format!("{parsed}\n") != row.expected
                || row.seat != "solver"
                || row.seeds.len() != 1
                || !seeds.insert(row.seeds[0])
                || !prompts.insert(row.prompt.clone())
                || !row.id.starts_with(&format!("{stage}-{}-", row.family))
            {
                return Err(Error::InvalidEvaluation);
            }
            *families.entry(row.family.clone()).or_insert(0_usize) += 1;
            tasks.push(PrivateTask::new(
                row.id,
                row.family,
                row.seat,
                row.prompt,
                HostVerifier::Golden {
                    expected: row.expected.into_bytes(),
                },
                row.seeds,
            )?);
        }
        if families.keys().map(String::as_str).collect::<Vec<_>>() != FAMILIES
            || families.values().collect::<BTreeSet<_>>().len() != 1
        {
            return Err(Error::InvalidEvaluation);
        }
        stages.insert(stage, tasks);
    }
    let mut take = |name: &str| stages.remove(name).ok_or(Error::InvalidEvaluation);
    PrivateSuites::freeze(
        take("screen")?,
        take("selection")?,
        take("confirmation")?,
        take("anchor")?,
    )
}

#[cfg(test)]
mod test;
