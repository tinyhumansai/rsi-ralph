//! Freeze exact private task identities and expose only observed build dispositions.
mod types;
use std::collections::{BTreeMap, BTreeSet};
use tinyrsi::{
    ArtifactSubmission, CommandSpec, HostVerifier, PrivateSuites, PrivateTask, VerificationBinding,
    VerifierDeclaration, VerifierRegistry,
};

/// Validate the stable core-only slice without exposing parse payloads.
fn decode(bytes: &[u8]) -> Result<types::Slice, Box<dyn std::error::Error>> {
    let slice: types::Slice =
        serde_json::from_slice(bytes).map_err(|_| "invalid private Lean slice")?;
    if slice.version != 1
        || slice.toolchain != "leanprover/lean4:v4.34.1"
        || slice.private_seed.len() != 64
        || !slice.private_seed.bytes().all(|b| b.is_ascii_hexdigit())
        || slice.stages.keys().map(String::as_str).collect::<Vec<_>>()
            != ["anchor", "confirmation", "screen", "selection"]
        || slice.stages.iter().any(|(stage, goals)| {
            let expected =
                ["euclid", "idempotent", "inverse"].map(|family| format!("{stage}-lean-{family}"));
            goals.len() != 3
                || goals.iter().map(|g| &g.id).collect::<BTreeSet<_>>() != expected.iter().collect()
        })
    {
        return Err("invalid private Lean slice".into());
    }
    let theorems = slice
        .stages
        .values()
        .flatten()
        .map(|goal| &goal.theorem)
        .collect::<BTreeSet<_>>();
    if theorems.len() != 12 {
        return Err("invalid private Lean slice".into());
    }
    Ok(slice)
}

/// Build the host's reference proofs; this is checker validation, not a model trial.
pub(super) fn cli(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() != 3 {
        return Err("usage: verify_lean_slice <actual-lake-binary> <private-slice.json>".into());
    }
    let slice = decode(&std::fs::read(&args[2])?)?;
    let command = CommandSpec {
        program: args[1].clone(),
        args: vec!["build".into()],
        workspace_root: "target/private-lean-workspaces".into(),
        timeout_ms: 20000,
        max_output_bytes: 65536,
    };
    let mut draws = BTreeMap::new();
    for (stage, goals) in &slice.stages {
        let tasks = goals
            .iter()
            .map(|goal| {
                PrivateTask::new(
                    goal.id.clone(),
                    "finite-mathematics".into(),
                    "solver".into(),
                    format!(
                        "Return a Lean proof block beginning with by for this frozen theorem:\n{}",
                        goal.theorem
                    ),
                    verifier(&command, &goal.theorem, &slice.toolchain),
                    vec![goal.seed],
                )
            })
            .collect::<tinyrsi::Result<Vec<_>>>()?;
        draws.insert(stage.clone(), tasks);
    }
    let mut take = |name: &str| draws.remove(name).ok_or("missing private stage");
    let suites = PrivateSuites::freeze(
        take("screen")?,
        take("selection")?,
        take("confirmation")?,
        take("anchor")?,
    )?;
    let mut receipts = Vec::new();
    for goals in slice.stages.values() {
        for goal in goals {
            let registry = VerifierRegistry::new(vec![VerifierDeclaration {
                node_id: "artifact".into(),
                owner_seat: "solver".into(),
                verifier: verifier(&command, &goal.theorem, &slice.toolchain),
            }])?;
            let binding = VerificationBinding {
                episode_id: format!("reference:{}", goal.id),
                seat_id: "solver".into(),
                node_id: "artifact".into(),
                artifact_id: "host-reference".into(),
                transcript_refs: vec![1],
            };
            let evidence = registry.verify(
                &binding,
                &ArtifactSubmission {
                    binding: binding.clone(),
                    content: b"by decide".to_vec(),
                },
            )?;
            if !evidence.passed {
                return Err("private reference proof did not pass".into());
            }
            receipts.push(types::Receipt {
                task_id: goal.id.clone(),
                passed: evidence.passed,
                exit_code: evidence.exit_code,
            });
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "version": 1, "kind": "host-reference checker validation; no model trials",
            "provenance": "agent-authored finite mathematics", "toolchain": slice.toolchain,
            "suite_fingerprints": suites.fingerprints(), "reference_builds": receipts,
        }))?
    );
    Ok(())
}

/// Apply the exact same host-frozen command and version to every stage.
fn verifier(command: &CommandSpec, theorem: &str, toolchain: &str) -> HostVerifier {
    HostVerifier::Lake {
        command: command.clone(),
        theorem: theorem.into(),
        toolchain: toolchain.into(),
    }
}

#[cfg(test)]
mod test;
