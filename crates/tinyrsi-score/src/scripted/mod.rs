//! A scorer written as a closure: scripted checks, fixtures and offline runs.
use crate::{Assessment, Result, Scorer};
use tinyrsi_types::Goal;

/// The closure behind a [`ScriptedScorer`].
type ScoreFn<S> = dyn Fn(&S, &Goal, &[u64]) -> Result<Assessment> + Send + Sync;

/// A scorer backed by a closure over `(subject, goal, seeds)`.
///
/// Use it for deterministic checks that need no model ("the answer equals
/// 42"), and for tests. Whether its assessment is accept-eligible is the
/// closure's decision, through [`Assessment::eligible`].
pub struct ScriptedScorer<S: ?Sized> {
    score: Box<ScoreFn<S>>,
}

impl<S: ?Sized> ScriptedScorer<S> {
    /// Wraps `score`.
    #[must_use]
    pub fn new(
        score: impl Fn(&S, &Goal, &[u64]) -> Result<Assessment> + Send + Sync + 'static,
    ) -> Self {
        Self {
            score: Box::new(score),
        }
    }

    /// An eligible scorer giving every seed `quality(subject)`, clamped to
    /// `[0, 1]`.
    #[must_use]
    pub fn uniform(quality: impl Fn(&S) -> f64 + Send + Sync + 'static) -> Self {
        Self::new(move |subject, _, seeds| {
            let q = quality(subject).clamp(0.0, 1.0);
            let qualities = vec![q; seeds.len()];
            Ok(Assessment {
                eligible: Some(qualities.clone()),
                qualities,
                metric: None,
                rationale: None,
                tokens: 0,
            })
        })
    }
}

impl<S: ?Sized> Scorer<S> for ScriptedScorer<S> {
    fn assess(&self, subject: &S, goal: &Goal, seeds: &[u64]) -> Result<Assessment> {
        (self.score)(subject, goal, seeds)
    }
}

impl<S: ?Sized> std::fmt::Debug for ScriptedScorer<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptedScorer").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test;
