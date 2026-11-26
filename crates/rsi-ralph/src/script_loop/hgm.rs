//! Huxley-Gödel Machine parent selection for [`super::Search::Hgm`].
//!
//! Every measured candidate is a node of the tree the ledger's `parent`
//! links already form. A node's *clade* is the node and all its descendants.
//! Before each proposal the loop draws, for every expandable node,
//! `theta ~ Beta(tau * (1 + s), tau * (1 + f))` over the clade's pooled
//! outcomes and edits the node with the largest draw: Thompson sampling on
//! clade metaproductivity, as in Wang et al., "Huxley-Gödel Machine" (2025)
//! and its reference code (`TS_sample` over `get_decendant_evals`).
//!
//! **Outcomes.** HGM scores a clade by the best agent it contains: how often
//! the family reaches the frontier. A script's metric is continuous, so each
//! seed of a node counts as a success when it beats the *champion of the
//! time* (the incumbent before the node's epoch) on that seed, half a success
//! when equal, and a failure when worse or failed. A fixed baseline-to-target
//! scale would saturate near the target, and beating the node's parent would
//! favour weak lines that are merely easy to improve; beating the champion
//! rewards families that keep producing new bests. The root contributes no
//! outcome of its own. Invalid and duplicate proposals add no node, as failed
//! expansions add none in HGM.
//!
//! **Cooling.** `tau` grows from 1 in the first epoch to `max_epochs` in the
//! last (`tau = E / (E - e + 1)`), the reference code's `cool_down` schedule
//! with exponent 1: draws start wide (explore) and narrow towards each
//! clade's mean as the budget runs out (exploit).
//!
//! Draws are deterministic for a run id, epoch and slot.
use super::{CandidateDecision, LedgerCandidate, ScriptLedger};
use crate::MetricDirection;
use std::collections::BTreeMap;

/// Pooled outcomes of one node's clade.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Clade {
    /// Seeds in the clade that beat the champion of their time (ties count
    /// half).
    pub successes: f64,
    /// Seeds in the clade that did not.
    pub failures: f64,
}

/// One seed's outcome against the reference value on the same seed.
fn outcome(direction: MetricDirection, parent: Option<f64>, metric: Option<f64>) -> f64 {
    let (Some(p), Some(m)) = (parent, metric) else {
        return 0.0;
    };
    let gain = match direction {
        MetricDirection::Minimize => p - m,
        MetricDirection::Maximize => m - p,
    };
    if gain > 0.0 {
        1.0
    } else if gain < 0.0 {
        0.0
    } else {
        0.5
    }
}

/// Clade statistics of every node, keyed by content id.
///
/// Nodes are the first measured row of each content id; the baseline is the
/// root. Each node's per-seed outcomes against the champion before its epoch
/// (the root when the ledger has no earlier incumbent) are added to itself
/// and every ancestor.
pub(super) fn clades(ledger: &ScriptLedger) -> BTreeMap<String, Clade> {
    let direction = ledger.metric.direction;
    let mut parents: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut runs: BTreeMap<String, Vec<Option<f64>>> = BTreeMap::new();
    let mut nodes: Vec<&LedgerCandidate> = vec![];
    for c in &ledger.candidates {
        let Some(id) = c.id.as_ref() else { continue };
        if c.runs.is_empty()
            || matches!(
                c.decision,
                CandidateDecision::Duplicate | CandidateDecision::Invalid
            )
            || parents.contains_key(id)
        {
            continue;
        }
        parents.insert(id.clone(), c.parent.clone());
        runs.insert(id.clone(), c.runs.iter().map(|r| r.metric).collect());
        nodes.push(c);
    }
    let mut out: BTreeMap<String, Clade> = parents
        .keys()
        .map(|k| (k.clone(), Clade::default()))
        .collect();
    let root = ledger.candidates.first().and_then(|c| c.id.clone());
    for c in nodes {
        if c.parent.is_none() {
            continue;
        }
        let champion = ledger
            .incumbents
            .iter()
            .rev()
            .find(|i| i.epoch < c.epoch)
            .map(|i| i.id.clone())
            .or_else(|| root.clone());
        let Some(before) = champion.as_ref().and_then(|id| runs.get(id)) else {
            continue;
        };
        let (mut s, mut f) = (0.0, 0.0);
        for (i, run) in c.runs.iter().enumerate() {
            let q = outcome(direction, before.get(i).copied().flatten(), run.metric);
            s += q;
            f += 1.0 - q;
        }
        let mut at = c.id.clone();
        let mut hops = 0;
        while let Some(id) = at {
            if let Some(clade) = out.get_mut(&id) {
                clade.successes += s;
                clade.failures += f;
            }
            at = parents.get(&id).cloned().flatten();
            hops += 1;
            if hops > parents.len() {
                break;
            }
        }
    }
    out
}

/// The cooling factor `tau` for `epoch` of `max_epochs`: 1 in the first
/// epoch, `max_epochs` in the last.
#[expect(
    clippy::cast_precision_loss,
    reason = "epoch counts stay far below 2^53"
)]
pub(super) fn cooling(epoch: u64, max_epochs: u64) -> f64 {
    let total = max_epochs.max(1) as f64;
    let left = max_epochs.saturating_sub(epoch.saturating_sub(1)).max(1) as f64;
    total / left
}

/// The node to expand among `expandable` (in order), by one Thompson draw
/// per node from its clade's Beta posterior, both parameters scaled by
/// `tau` (see [`cooling`]). `None` when `expandable` is empty.
pub(super) fn select<'a>(
    clades: &BTreeMap<String, Clade>,
    expandable: &[&'a str],
    seed: u64,
    tau: f64,
) -> Option<&'a str> {
    let mut rng = SplitMix(seed);
    let mut best: Option<(&str, f64)> = None;
    for &id in expandable {
        let clade = clades.get(id).copied().unwrap_or_default();
        let theta = beta(
            &mut rng,
            tau * (1.0 + clade.successes),
            tau * (1.0 + clade.failures),
        );
        if best.is_none_or(|(_, b)| theta > b) {
            best = Some((id, theta));
        }
    }
    best.map(|(id, _)| id)
}

/// A per-draw seed from the run id, epoch and slot.
pub(super) fn draw_seed(run_id: &str, epoch: u64, slot: u32) -> u64 {
    // FNV-1a over the run id, mixed with the epoch and slot.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in run_id.bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h ^ (epoch << 20) ^ u64::from(slot)
}

/// `SplitMix64`: a small, deterministic generator, enough for sampling.
struct SplitMix(u64);

impl SplitMix {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `(0, 1)`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "53 random bits map exactly onto an f64 mantissa"
    )]
    fn uniform(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// Standard normal by Box-Muller.
    fn normal(&mut self) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    /// `Gamma(shape, 1)` by Marsaglia and Tsang, boosted below shape 1.
    fn gamma(&mut self, shape: f64) -> f64 {
        if shape < 1.0 {
            let boost = self.uniform().powf(1.0 / shape);
            return self.gamma(shape + 1.0) * boost;
        }
        let offset = shape - 1.0 / 3.0;
        let spread = 1.0 / (9.0 * offset).sqrt();
        loop {
            let draw = self.normal();
            let cube = (1.0 + spread * draw).powi(3);
            if cube <= 0.0 {
                continue;
            }
            let accept = self.uniform().ln();
            if accept < 0.5 * draw * draw + offset - offset * cube + offset * cube.ln() {
                return offset * cube;
            }
        }
    }
}

/// `Beta(a, b)` from two gamma draws.
fn beta(rng: &mut SplitMix, a: f64, b: f64) -> f64 {
    let x = rng.gamma(a);
    let y = rng.gamma(b);
    x / (x + y)
}
