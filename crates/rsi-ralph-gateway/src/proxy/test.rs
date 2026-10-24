//! Unit tests for tag extraction, error mapping, capture names and redaction.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::http::{HeaderMap, HeaderValue, StatusCode};

use super::{ProxyConfig, capture_name, error_status, header_tags, path_tags, redact};
use crate::error::Error;
use crate::price::PriceTable;
use crate::request::Limits;

#[test]
fn reads_tags_from_headers_and_paths() {
    let mut headers = HeaderMap::new();
    headers.insert("x-tinyrsi-phase", HeaderValue::from_static("evolve"));
    headers.insert("x-tinyrsi-seat", HeaderValue::from_static("lead"));
    headers.insert("x-tinyrsi-candidate", HeaderValue::from_static("c1"));
    let tags = header_tags(&headers);
    assert_eq!(
        (
            tags.phase.as_str(),
            tags.seat.as_str(),
            tags.candidate.as_str()
        ),
        ("evolve", "lead", "c1")
    );
    let tags = path_tags(&headers, "score".into(), "judge".into());
    assert_eq!(
        (
            tags.phase.as_str(),
            tags.seat.as_str(),
            tags.candidate.as_str()
        ),
        ("score", "judge", "c1")
    );
    assert_eq!(header_tags(&HeaderMap::new()).phase, "");
}

#[test]
fn maps_errors_to_statuses() {
    let budget = Error::BudgetExceeded {
        ledger: "run",
        needed: 1.0,
        remaining: 0.0,
    };
    assert_eq!(error_status(&budget), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(
        error_status(&Error::PriceBoundBreached),
        StatusCode::PAYMENT_REQUIRED
    );
    assert_eq!(
        error_status(&Error::UnknownModel("x".into())),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        error_status(&Error::Upstream("x".into())),
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(
        error_status(&Error::BadRequest("x".into())),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        error_status(&Error::ServerStopped),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn capture_names_cannot_escape_their_directory() {
    assert_eq!(capture_name(7, "score"), "000007-score.json");
    assert_eq!(capture_name(0, ""), "000000-untagged.json");
    assert_eq!(capture_name(1, "../../etc"), "000001-______etc.json");
}

#[test]
fn redacts_the_secret_wherever_it_appears() {
    assert_eq!(
        redact("bad key sk-1 (sk-1)", "sk-1"),
        "bad key [redacted] ([redacted])"
    );
    assert_eq!(redact("nothing", ""), "nothing");
}

#[test]
fn debug_output_hides_the_key() {
    let config = ProxyConfig {
        upstream: "http://127.0.0.1:9/v1".into(),
        api_key: "sk-secret".into(),
        prices: PriceTable::standard(),
        limits: Limits::default(),
        capture_dir: None,
    };
    let debug = format!("{config:?}");
    assert!(!debug.contains("sk-secret"), "{debug}");
    assert!(debug.contains("[redacted]"));
}

/// A refused connection is eligible, while an invalid request is not.
#[tokio::test]
async fn retries_only_connection_establishment_errors() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let refused = client
        .post(format!("http://{address}/"))
        .send()
        .await
        .unwrap_err();
    assert!(super::before_delivery(&refused));
    let invalid = client.post("not a url").send().await.unwrap_err();
    assert!(!super::before_delivery(&invalid));
}
