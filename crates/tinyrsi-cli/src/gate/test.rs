//! The surface check and gate command sequencing.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::manifest::SubjectKind;
use std::collections::BTreeMap;

fn spec(editable: &[&str], frozen: &[&str], tests: bool) -> SubjectSpec {
    SubjectSpec {
        kind: SubjectKind::Git,
        path: "/r".into(),
        branch: Some("main".into()),
        editable: editable.iter().map(|s| (*s).to_owned()).collect(),
        frozen: frozen.iter().map(|s| (*s).to_owned()).collect(),
        require_test_change: tests,
    }
}

fn changed(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn accepts_changes_inside_editable_prefixes() {
    let s = spec(&["crates/tools/", "coloring.py"], &["crates/tools/api/"], false);
    assert!(surface(&s, &changed(&["crates/tools/src/a.rs", "coloring.py"])).ok);
    assert!(surface(&s, &changed(&[])).ok);
}

#[test]
fn rejects_frozen_and_outside_paths() {
    let s = spec(&["crates/tools/", "coloring.py"], &["crates/tools/api/"], false);
    let out = surface(&s, &changed(&["crates/tools/api/x.rs"]));
    assert!(!out.ok);
    assert_eq!(out.reason.as_deref(), Some("crates/tools/api/x.rs is outside the editable surface"));
    assert!(!surface(&s, &changed(&["crates/toolsmith/a.rs"])).ok);
    assert!(!surface(&s, &changed(&["coloring.py.bak"])).ok);
    let dir = spec(&["src"], &[], false);
    assert!(surface(&dir, &changed(&["src/a.rs"])).ok);
}

#[test]
fn can_require_a_test_change() {
    let s = spec(&["src/", "tests/"], &[], true);
    assert_eq!(surface(&s, &changed(&["src/a.rs"])).reason.as_deref(), Some("no test file changed"));
    assert!(surface(&s, &changed(&["src/a.rs", "tests/a_test.rs"])).ok);
}

#[test]
fn gates_run_in_order_and_stop_at_the_first_failure() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = StepContext {
        loop_dir: dir.path().to_path_buf(),
        run_dir: dir.path().to_path_buf(),
        incumbent: dir.path().to_path_buf(),
        epoch: 1,
        model_url: None,
        env: BTreeMap::new(),
        inherited: vec![("PATH".into(), "/usr/bin:/bin".into())],
    };
    let cand = Checkout { label: "c".into(), path: dir.path().to_path_buf() };
    let gates = vec![
        GateStep { run: "true".into(), cwd: StepCwd::Candidate, timeout_secs: 10 },
        GateStep { run: "false".into(), cwd: StepCwd::Loop, timeout_secs: 10 },
        GateStep { run: "true".into(), cwd: StepCwd::Loop, timeout_secs: 10 },
    ];
    let outcomes = run_gates(&ctx, &gates, &cand, &dir.path().join("gates")).unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes[0].ok && !outcomes[1].ok);
    assert_eq!(outcomes[1].id, "gate.1");
}
