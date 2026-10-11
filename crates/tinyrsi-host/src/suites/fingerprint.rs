//! Content fingerprints bind private tasks to their actual host checker settings.
use super::PrivateTask;
use crate::{CommandSpec, Error, HostVerifier, Result};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
fn command(command: &CommandSpec) -> Result<Value> {
    let executable = std::fs::read(&command.program).map_err(|_| Error::HostFailed)?;
    Ok(
        json!({"program":command.program,"executable":digest(&executable),"args":command.args,"workspace":command.workspace_root,"timeout":command.timeout_ms,"output":command.max_output_bytes}),
    )
}
pub(super) fn checker(verifier: &HostVerifier) -> Result<String> {
    let value = match verifier {
        HostVerifier::Golden { expected } => json!({"golden":expected}),
        HostVerifier::Schema { schema } => json!({"schema":schema}),
        HostVerifier::Process(c) => command(c)?,
        HostVerifier::Lean {
            command: c,
            theorem,
        } => json!({"lean":command(c)?,"theorem":theorem}),
        HostVerifier::Lake {
            command: c,
            theorem,
            toolchain,
        } => json!({
            "lake": command(c)?, "theorem": theorem, "toolchain": toolchain,
            "project": tinyrsi_verify::lake_identity(c)?
        }),
    };
    Ok(digest(
        &serde_json::to_vec(&value).map_err(|_| Error::InvalidEvaluation)?,
    ))
}
pub(super) fn suite(stage: &str, tasks: &[PrivateTask]) -> Result<String> {
    let tasks:Vec<_>=tasks.iter().map(|t|json!({"id":t.id,"family":t.family,"seat":t.seat,"prompt":t.prompt,"checker":t.checker_stamp,"seeds":t.seeds})).collect();
    Ok(digest(
        &serde_json::to_vec(&json!({"stage":stage,"tasks":tasks,"goal_weight":0.5,"verifier_tiers":"proof:1,execution:0.9,schema:0.7,reference:0.7"}))
            .map_err(|_| Error::InvalidEvaluation)?,
    ))
}
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
