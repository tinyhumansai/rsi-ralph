//! Host-owned verified decomposition; configuration never changes during reports.
mod types;
use crate::{Error, Result};
use std::collections::BTreeSet;
pub use types::{BlueprintNode, NodeStatus, Verifier, VerifierKind};

/// A validated dependency graph whose results are supplied by the host.
#[derive(Clone, Debug)]
pub struct Blueprint {
    nodes: Vec<BlueprintNode>,
}
impl Blueprint {
    /// Validates identities, configuration, dependencies and acyclicity.
    ///
    /// # Errors
    /// Returns [`Error::InvalidBlueprint`] for an empty or invalid graph,
    /// including nodes already marked verified without a host report.
    pub fn new(nodes: Vec<BlueprintNode>) -> Result<Self> {
        let ids: BTreeSet<_> = nodes.iter().map(|node| node.id.as_str()).collect();
        if nodes.is_empty()
            || ids.len() != nodes.len()
            || nodes.iter().any(|node| {
                [
                    &node.id,
                    &node.statement,
                    &node.owner_role,
                    &node.verifier.spec,
                ]
                .iter()
                .any(|value| value.trim().is_empty())
                    || node.status == NodeStatus::Verified
                    || node.dependencies.iter().collect::<BTreeSet<_>>().len()
                        != node.dependencies.len()
                    || node
                        .dependencies
                        .iter()
                        .any(|id| !ids.contains(id.as_str()))
            })
        {
            return Err(Error::InvalidBlueprint);
        }
        let mut visited = BTreeSet::new();
        loop {
            let before = visited.len();
            for node in &nodes {
                if node.dependencies.iter().all(|id| visited.contains(id)) {
                    visited.insert(node.id.clone());
                }
            }
            if visited.len() == nodes.len() {
                break;
            }
            if before == visited.len() {
                return Err(Error::InvalidBlueprint);
            }
        }
        Ok(Self { nodes })
    }
    /// Immutable nodes in their supplied order.
    #[must_use]
    pub fn nodes(&self) -> &[BlueprintNode] {
        &self.nodes
    }
    /// Unverified nodes whose dependencies have host-confirmed passes.
    #[must_use]
    pub fn ready(&self) -> Vec<&BlueprintNode> {
        self.nodes
            .iter()
            .filter(|node| node.status != NodeStatus::Verified && self.dependencies_verified(node))
            .collect()
    }
    /// Applies an external verifier outcome; a failed prerequisite revokes
    /// dependent passes, which must be verified again.
    ///
    /// # Errors
    /// Returns [`Error::InvalidBlueprint`] for an unknown node, or a pass
    /// whose prerequisites are unverified. Configuration is never rewritten.
    pub fn report(&mut self, id: &str, passed: bool) -> Result<()> {
        let index = self
            .nodes
            .iter()
            .position(|node| node.id == id)
            .ok_or(Error::InvalidBlueprint)?;
        if passed && !self.dependencies_verified(&self.nodes[index]) {
            return Err(Error::InvalidBlueprint);
        }
        self.nodes[index].status = if passed {
            NodeStatus::Verified
        } else {
            NodeStatus::Failed
        };
        if !passed {
            loop {
                let invalid: Vec<_> = self
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| {
                        node.status == NodeStatus::Verified && !self.dependencies_verified(node)
                    })
                    .map(|(index, _)| index)
                    .collect();
                if invalid.is_empty() {
                    break;
                }
                for index in invalid {
                    self.nodes[index].status = NodeStatus::Open;
                }
            }
        }
        Ok(())
    }
    /// Whether every node has a host-confirmed verifier pass.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.nodes.is_empty()
            && self
                .nodes
                .iter()
                .all(|node| node.status == NodeStatus::Verified)
    }
    fn dependencies_verified(&self, node: &BlueprintNode) -> bool {
        node.dependencies.iter().all(|id| {
            self.nodes
                .iter()
                .any(|dependency| dependency.id == *id && dependency.status == NodeStatus::Verified)
        })
    }
}
#[cfg(test)]
mod test;
