//! Dependency validation and host verifier reports.
#![allow(clippy::unwrap_used)]
use super::*;

fn node(id: &str, dependencies: &[&str]) -> BlueprintNode {
    BlueprintNode {
        id: id.into(),
        statement: format!("prove {id}"),
        owner_role: "worker".into(),
        dependencies: dependencies.iter().map(|id| (*id).into()).collect(),
        verifier: Verifier {
            kind: VerifierKind::Execution,
            spec: "tests pass".into(),
        },
        status: NodeStatus::Open,
    }
}
#[test]
fn rejects_invalid_graphs_and_preverified_nodes() {
    assert!(Blueprint::new(vec![]).is_err());
    for nodes in [
        vec![node("a", &[]), node("a", &[])],
        vec![node("a", &["missing"])],
        vec![node("a", &["b"]), node("b", &["a"])],
        vec![node("a", &["a"])],
        vec![node("", &[])],
        vec![node("a", &["b", "b"]), node("b", &[])],
    ] {
        assert_eq!(
            Blueprint::new(nodes).unwrap_err(),
            crate::Error::InvalidBlueprint
        );
    }
    let mut invalid = node("a", &[]);
    invalid.status = NodeStatus::Verified;
    assert!(Blueprint::new(vec![invalid]).is_err());
    for field in 0..3 {
        let mut invalid = node("a", &[]);
        match field {
            0 => invalid.statement.clear(),
            1 => invalid.owner_role.clear(),
            _ => invalid.verifier.spec.clear(),
        }
        assert!(Blueprint::new(vec![invalid]).is_err());
    }
}
#[test]
fn only_explicit_verifier_passes_unlock_dependencies() {
    let mut graph = Blueprint::new(vec![node("a", &[]), node("b", &["a"])]).unwrap();
    assert!(!graph.is_complete());
    assert_eq!(graph.ready()[0].id, "a");
    assert!(graph.report("b", true).is_err());
    assert!(graph.report("unknown", false).is_err());
    graph.report("a", false).unwrap();
    assert_eq!(graph.nodes()[0].status, NodeStatus::Failed);
    graph.report("a", true).unwrap();
    assert_eq!(graph.ready()[0].id, "b");
    graph.report("b", true).unwrap();
    assert!(graph.is_complete());
    graph.report("a", false).unwrap();
    assert!(!graph.is_complete());
    assert_eq!(graph.nodes()[1].status, NodeStatus::Open);
}
#[test]
fn sorry_and_failed_nodes_remain_ready_and_ladder_flags_weak_evidence() {
    let mut sorry = node("a", &[]);
    sorry.status = NodeStatus::Sorry;
    assert_eq!(Blueprint::new(vec![sorry]).unwrap().ready().len(), 1);
    assert!(VerifierKind::Proof < VerifierKind::Execution);
    assert!(VerifierKind::Execution < VerifierKind::Schema);
    assert!(VerifierKind::Schema < VerifierKind::HeldOutReference);
    assert!(VerifierKind::HeldOutReference < VerifierKind::Rubric);
    assert!(VerifierKind::Rubric < VerifierKind::Preference);
    assert!(!VerifierKind::Proof.is_weak());
    assert!(VerifierKind::Rubric.is_weak());
    assert!(VerifierKind::Preference.is_weak());
}

#[test]
fn prerequisite_failure_revokes_transitive_passes() {
    let mut graph =
        Blueprint::new(vec![node("c", &["b"]), node("b", &["a"]), node("a", &[])]).unwrap();
    for id in ["a", "b", "c"] {
        graph.report(id, true).unwrap();
    }
    graph.report("a", false).unwrap();
    assert_eq!(graph.nodes()[0].status, NodeStatus::Open);
    assert_eq!(graph.nodes()[1].status, NodeStatus::Open);
    assert_eq!(
        graph
            .ready()
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["a"]
    );
}
