//! Curve fitting: lower the held-out RMSE of a noisy damped sinusoid fit.
//!
//! The fixtures stay with the self-contained `rsi-ralph` example in
//! `crates/rsi-ralph/examples/script_loop/` and are embedded from there, brief
//! included, so both runs read one copy. The canned replies match that
//! example's: a cubic (accepted), a constant (rejected), an edit to the fixed
//! harness (invalid) and a damped-sinusoid fit (reaches the target).
use super::{Definition, metric};
use crate::{Replies, proposal};
use rsi_ralph::{LoopPolicy, MetricDirection};

const HARNESS: &str = include_str!("../../../crates/rsi-ralph/examples/script_loop/curve_fit.py");
const MODEL: &str = include_str!("../../../crates/rsi-ralph/examples/script_loop/model.py");
const BRIEF: &str = include_str!("../../../crates/rsi-ralph/examples/script_loop/brief.txt");
const CUBIC: &str = include_str!("../../../crates/rsi-ralph/examples/script_loop/proposals/cubic.py");
const CONSTANT: &str =
    include_str!("../../../crates/rsi-ralph/examples/script_loop/proposals/constant.py");
const DAMPED: &str =
    include_str!("../../../crates/rsi-ralph/examples/script_loop/proposals/damped.py");

pub(super) fn definition() -> Definition {
    Definition {
        name: "curve_fit.py",
        fixtures: &[("curve_fit.py", HARNESS), ("model.py", MODEL)],
        editable: "model.py",
        checker: "curve_fit.py",
        brief: BRIEF.trim_end().into(),
        metric: metric("rmse", MetricDirection::Minimize, 0.06),
        seeds: vec![1, 2, 3],
        timeout_ms: 60_000,
        // The `script_loop` example's defaults.
        policy: LoopPolicy {
            max_epochs: 4,
            proposals_per_epoch: 2,
            patience: 3,
            token_budget: 400_000,
            ..LoopPolicy::default()
        },
    }
}

pub(super) fn canned() -> Replies {
    Replies::new([
        proposal(
            "The straight line is undertrained and cannot bend with the signal.",
            "An exact cubic least-squares fit lowers held-out RMSE.",
            "model.py",
            CUBIC,
        ),
        proposal(
            "Noise dominates; a constant is the most robust predictor.",
            "Predicting the training peak lowers RMSE.",
            "model.py",
            CONSTANT,
        ),
        proposal(
            "Change the harness to evaluate on training points instead.",
            "Scoring on training data lowers RMSE.",
            "../curve_fit.py",
            HARNESS,
        ),
        proposal(
            "The residuals oscillate with a decaying envelope: fit that family.",
            "A damped sinusoid fit reaches the noise floor of about 0.05.",
            "model.py",
            DAMPED,
        ),
    ])
}
