//! What an improvement loop is trying to achieve, in words.
//!
//! A [`Goal`] is what a judge reads: a statement and the criteria a better
//! subject meets. Numeric targets belong to the scorer that measures them; a
//! goal is deliberately just text, because many goals are vague.
use serde::{Deserialize, Serialize};

/// A goal stated for judges: what "better" means, and how to tell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    /// One-sentence statement of the goal.
    pub statement: String,
    /// Checkable criteria a better subject meets, most important first.
    pub criteria: Vec<String>,
}

impl Goal {
    /// A goal with no criteria yet.
    ///
    /// # Examples
    ///
    /// ```
    /// use rsi_ralph_types::Goal;
    /// let goal = Goal::new("Explain the proof clearly").with_criterion("every step is justified");
    /// assert_eq!(goal.criteria.len(), 1);
    /// ```
    #[must_use]
    pub fn new(statement: impl Into<String>) -> Self {
        Self {
            statement: statement.into(),
            criteria: vec![],
        }
    }

    /// Adds one criterion.
    #[must_use]
    pub fn with_criterion(mut self, criterion: impl Into<String>) -> Self {
        self.criteria.push(criterion.into());
        self
    }

    /// Whether the statement is present and every criterion is non-empty.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        !self.statement.trim().is_empty() && self.criteria.iter().all(|c| !c.trim().is_empty())
    }
}

#[cfg(test)]
mod test;
