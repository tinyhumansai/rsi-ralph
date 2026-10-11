//! Known numeric results and explicit censoring rules for descriptive metrics.
#![expect(clippy::unwrap_used, reason = "assert deterministic metric fixtures")]
use super::*;
fn dataset(gains: &[f64], deltas: &[f64]) -> ProposalOutcomeDataset {
    ProposalOutcomeDataset {
        version: 1,
        epoch: 1,
        fingerprint: "a".repeat(64),
        mode: crate::GenerationMode::Direct,
        model: None,
        generation_seed: None,
        generation: None,
        meter: crate::GenerationMeter::default(),
        generation_complete: true,
        rows: gains
            .iter()
            .zip(deltas)
            .enumerate()
            .map(|(i, (gain, delta))| ProposalOutcomeRow {
                proposal_index: i,
                occurrence: None,
                formatter_run_id: None,
                ranked: true,
                predicted_gain: *gain,
                prior: Some(0.5),
                stage: ProposalStage::Selection,
                reason: None,
                candidate_id: None,
                label: ProposalLabel::Observed,
                selection: Some(ProposalSelection {
                    delta: *delta,
                    lower: *delta - 0.01,
                    upper: *delta + 0.01,
                    eligible: Some(*delta > 0.0),
                }),
                interrupted: false,
            })
            .collect(),
    }
}
#[test]
fn brier_and_top_k_use_selection_eligibility_not_approval() {
    let mut d = dataset(&[4.0, 3.0, 2.0, 1.0], &[0.4, 0.3, -0.2, -0.1]);
    for (r, p) in d.rows.iter_mut().zip([0.9, 0.8, 0.2, 0.1]) {
        r.prior = Some(p);
        r.reason = Some(crate::ReasonCode::ApprovalDenied);
    }
    let m = d.metrics(2).unwrap();
    assert!((m.brier.unwrap() - 0.025).abs() < 1e-12);
    assert_eq!(m.brier_pairs, 4);
    assert_eq!(m.precision_at_k, Some(1.0));
    assert_eq!(m.missing_labels, 0);
    assert_eq!(m.missing_priors, 0);
    assert_eq!(d.metrics(4).unwrap().precision_at_k, Some(0.5));
    let json = serde_json::to_string(&d).unwrap();
    assert_eq!(
        serde_json::from_str::<ProposalOutcomeDataset>(&json).unwrap(),
        d
    );
}
#[test]
fn spearman_uses_average_tied_ranks_and_detects_constant_vectors() {
    for (x, y, expected) in [
        (vec![1.0, 2.0, 3.0], vec![1.0, 2.0, 3.0], 1.0),
        (vec![1.0, 2.0, 3.0], vec![3.0, 2.0, 1.0], -1.0),
        (vec![1.0, 1.0, 2.0], vec![1.0, 2.0, 3.0], 0.75_f64.sqrt()),
        (vec![f64::MAX, -f64::MAX, 0.0], vec![3.0, 1.0, 2.0], 1.0),
    ] {
        assert!((dataset(&x, &y).metrics(1).unwrap().spearman.unwrap() - expected).abs() < 1e-12);
    }
    assert!(
        dataset(&[1.0, 1.0], &[1.0, 2.0])
            .metrics(1)
            .unwrap()
            .spearman
            .is_none()
    );
    assert!(
        dataset(&[1.0, 2.0], &[1.0, 1.0])
            .metrics(1)
            .unwrap()
            .spearman
            .is_none()
    );
    assert!(
        dataset(&[1.0], &[1.0])
            .metrics(1)
            .unwrap()
            .spearman
            .is_none()
    );
    assert!(
        dataset(&[-0.0, 0.0], &[1.0, 2.0])
            .metrics(1)
            .unwrap()
            .spearman
            .is_none()
    );
}
#[test]
fn censored_and_unresolved_labels_never_become_negative_or_drop_from_top_k() {
    let mut d = dataset(&[4.0, 3.0, 2.0, 1.0], &[0.4, 0.3, 0.2, -0.1]);
    d.rows[0].selection = None;
    d.rows[0].label = ProposalLabel::Censored;
    d.rows[1].selection.as_mut().unwrap().eligible = None;
    d.rows[1].label = ProposalLabel::Inconclusive;
    d.rows[2].prior = None;
    let m = d.metrics(2).unwrap();
    assert_eq!(m.brier_pairs, 1);
    assert_eq!(m.brier, Some(0.25));
    assert_eq!(m.gain_pairs, 3);
    assert_eq!(m.missing_labels, 2);
    assert_eq!(m.missing_priors, 1);
    assert_eq!(m.top_k_observed, 0);
    assert!(m.precision_at_k.is_none());
    for r in &mut d.rows {
        r.selection = None;
        r.label = ProposalLabel::Pending;
    }
    let m = d.metrics(1).unwrap();
    assert!(m.brier.is_none());
    assert!(m.spearman.is_none());
    assert_eq!(m.brier_pairs, 0);
    assert_eq!(m.gain_pairs, 0);
}
#[test]
fn ties_in_top_k_keep_proposal_order_including_missing_labels() {
    let mut d = dataset(&[1.0, 1.0], &[1.0, 1.0]);
    d.rows[0].selection = None;
    d.rows[0].label = ProposalLabel::Interrupted;
    assert!(d.metrics(1).unwrap().precision_at_k.is_none());
    d.rows[0].predicted_gain = -0.0;
    d.rows[1].predicted_gain = 0.0;
    assert!(d.metrics(1).unwrap().precision_at_k.is_none());
}
#[test]
fn malformed_deserialized_datasets_are_not_scored() {
    let d = dataset(&[1.0, 2.0], &[1.0, 2.0]);
    assert!(d.metrics(0).is_err());
    assert!(d.metrics(3).is_err());
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut c = d.clone();
        c.rows[0].predicted_gain = bad;
        assert!(c.metrics(1).is_err());
        let mut c = d.clone();
        c.rows[0].selection.as_mut().unwrap().delta = bad;
        assert!(c.metrics(1).is_err());
    }
    for bad in [f64::NAN, -0.1, 1.1] {
        let mut c = d.clone();
        c.rows[0].prior = Some(bad);
        assert!(c.metrics(1).is_err());
    }
    for field in ["version", "fingerprint", "rows"] {
        let mut c = serde_json::to_value(&d).unwrap();
        match field {
            "version" => c[field] = serde_json::json!(2),
            "fingerprint" => c[field] = serde_json::json!("bad"),
            _ => c[field][0]["proposal_index"] = serde_json::json!(4),
        }
        assert!(
            serde_json::from_value::<ProposalOutcomeDataset>(c)
                .unwrap()
                .metrics(1)
                .is_err()
        );
    }
    let mut c = d.clone();
    c.rows[0].label = ProposalLabel::Duplicate;
    assert!(c.metrics(1).is_err());
    let mut c = d.clone();
    c.rows[0].selection = None;
    assert!(c.metrics(1).is_err());
    let mut c = d.clone();
    c.rows[0].label = ProposalLabel::Inconclusive;
    assert!(c.metrics(1).is_err());
    let mut c = d.clone();
    c.rows[0].selection.as_mut().unwrap().eligible = None;
    assert!(c.metrics(1).is_err());
    let mut c = d.clone();
    c.rows[0].selection.as_mut().unwrap().lower = 10.0;
    assert!(c.metrics(1).is_err());
    let mut c = d.clone();
    c.rows[0].formatter_run_id = Some("not-an-id".into());
    assert!(c.metrics(1).is_err());
    let mut c = d.clone();
    c.model = Some(crate::ModelIdentity {
        provider: String::new(),
        model: "secret payload".into(),
    });
    assert!(c.metrics(1).is_err());
    let mut c = d;
    c.rows[0].occurrence = Some(ProposalOccurrence {
        call_id: 1,
        item_index: 0,
    });
    assert!(c.metrics(1).is_err());
}
