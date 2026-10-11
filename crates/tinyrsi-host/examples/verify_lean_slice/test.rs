//! Private slice parsing never reports protected theorem or seed payloads.
use serde_json::json;

#[test]
fn rejects_unknown_or_missing_stages_and_does_not_expose_private_parse_errors() {
    let stages = ["screen", "selection", "confirmation", "anchor"]
        .into_iter()
        .enumerate()
        .map(|(i, stage)| {
            (stage.to_string(), json!(["euclid", "idempotent", "inverse"].into_iter().enumerate().map(|(j, family)|
                json!({"id": format!("{stage}-lean-{family}"), "seed":i*3+j, "theorem":format!("private-canary-{stage}-{family}")})
            ).collect::<Vec<_>>()))
        })
        .collect::<serde_json::Map<_, _>>();
    let base = json!({"version":1,"private_seed":"ab".repeat(32),"toolchain":"leanprover/lean4:v4.34.1","stages":stages});
    assert!(super::decode(&serde_json::to_vec(&base).unwrap()).is_ok());
    for pointer in ["/version", "/private_seed", "/toolchain", "/stages/screen"] {
        let mut invalid = base.clone();
        *invalid.pointer_mut(pointer).unwrap() = json!(null);
        let error = super::decode(&serde_json::to_vec(&invalid).unwrap())
            .err()
            .unwrap();
        assert!(!error.to_string().contains("private-canary"));
    }
    let mut overlapping = base.clone();
    overlapping["stages"]["selection"][0]["theorem"] =
        base["stages"]["screen"][0]["theorem"].clone();
    assert!(super::decode(&serde_json::to_vec(&overlapping).unwrap()).is_err());
    assert!(super::cli(&[]).is_err());
    assert!(super::super::main().is_err());
}
