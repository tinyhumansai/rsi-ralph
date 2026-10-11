//! Validate actual suite freezing, strict formats and private error redaction.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use serde_json::{Value, json};

fn fixture() -> Value {
    let stages = super::STAGES
        .into_iter()
        .enumerate()
        .map(|(s, stage)| {
            let rows = super::FAMILIES
                .into_iter()
                .enumerate()
                .map(|(f, family)| {
                    json!({
                        "id": format!("{stage}-{family}-0"), "family": family,
                        "seat": "solver", "prompt": format!("private-canary-{stage}-{family}"),
                        "expected": "42\n", "seeds": [s * 4 + f]
                    })
                })
                .collect::<Vec<_>>();
            (stage.to_string(), Value::Array(rows))
        })
        .collect::<serde_json::Map<_, _>>();
    let calibration = stages
        .values()
        .flat_map(|rows| rows.as_array().unwrap())
        .flat_map(|row| {
            [
                json!({"task_id": row["id"], "artifact": "42\n", "passed": true}),
                json!({"task_id": row["id"], "artifact": "43\n", "passed": false}),
            ]
        })
        .collect::<Vec<_>>();
    json!({"version": 1, "provenance": "agent-authored; two independent integer oracles",
           "private_seed": "ab".repeat(32), "stages": stages, "calibration": calibration})
}

#[test]
fn freezes_actual_golden_suites_without_exporting_private_inputs() {
    let bank = fixture();
    let suites = super::freeze(&serde_json::to_vec(&bank).unwrap()).unwrap();
    let public = serde_json::to_string(&suites.fingerprints()).unwrap();
    assert_eq!(suites.fingerprints().len(), 4);
    assert!(!public.contains("private-canary"));
    assert!(!public.contains("expected"));
    assert!(!format!("{suites:?}").contains("private-canary"));
    assert_eq!(
        super::freeze(&serde_json::to_vec(&bank).unwrap())
            .unwrap()
            .fingerprints(),
        suites.fingerprints()
    );
}

#[test]
fn rejects_malformed_or_reused_draws_and_noncanonical_answers() {
    let base = fixture();
    for (pointer, replacement) in [
        ("/version", json!(2)),
        ("/private_seed", json!("short")),
        ("/provenance", json!("human-rated")),
        ("/stages/screen", json!([])),
        ("/stages/screen/0/expected", json!("042\n")),
        ("/stages/screen/0/expected", json!("42")),
        ("/stages/screen/0/seeds", json!([0])),
        ("/stages/screen/0/seeds", json!([])),
        ("/stages/screen/0/seat", json!("other")),
        ("/stages/screen/0/family", json!("fake")),
        (
            "/stages/screen/0/prompt",
            base["stages"]["anchor"][0]["prompt"].clone(),
        ),
    ] {
        let mut bank = base.clone();
        *bank.pointer_mut(pointer).unwrap() = replacement;
        let error = super::freeze(&serde_json::to_vec(&bank).unwrap()).unwrap_err();
        assert!(!error.to_string().contains("private-canary"));
    }
    assert!(super::freeze(b"private-canary-invalid-json").is_err());
}

#[test]
fn offline_cli_reads_only_the_selected_bank_and_requires_one_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("private.json");
    std::fs::write(&path, serde_json::to_vec(&fixture()).unwrap()).unwrap();
    assert!(super::cli(&["example".into(), path.to_string_lossy().into_owned()]).is_ok());
    assert!(super::cli(&[]).is_err());
    assert!(
        super::cli(&[
            "example".into(),
            dir.path().join("missing").to_string_lossy().into_owned()
        ])
        .is_err()
    );
    std::fs::write(&path, b"private-canary-invalid-json").unwrap();
    let error = super::cli(&["example".into(), path.to_string_lossy().into_owned()]).unwrap_err();
    assert!(!error.to_string().contains("private-canary"));
    assert!(super::super::main().is_err());
}

#[test]
fn automatic_labels_require_actual_checker_observations_for_both_artifacts() {
    let base = fixture();
    assert_eq!(
        super::calibration::verify(&serde_json::to_vec(&base).unwrap()).unwrap(),
        32
    );
    for (pointer, replacement) in [
        ("/calibration", json!([])),
        ("/calibration/0/task_id", json!("absent")),
        ("/calibration/0/artifact", json!("wrong")),
        ("/calibration/0/passed", json!(false)),
    ] {
        let mut bank = base.clone();
        *bank.pointer_mut(pointer).unwrap() = replacement;
        assert!(super::calibration::verify(&serde_json::to_vec(&bank).unwrap()).is_err());
    }
    let mut duplicate = base.clone();
    duplicate["calibration"]
        .as_array_mut()
        .unwrap()
        .push(base["calibration"][0].clone());
    assert!(super::calibration::verify(&serde_json::to_vec(&duplicate).unwrap()).is_err());
}
