//! Unit tests for the gateway handle's configuration, URLs and spend.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{
    DEFAULT_LISTEN, DEFAULT_UPSTREAM, Gateway, GatewayConfig, Spend, encode_segment, refuse,
    wait_ready,
};
use crate::error::Error;

fn config(dir: &std::path::Path) -> GatewayConfig {
    GatewayConfig::new(
        "sk-unit-secret",
        dir.join("run"),
        dir.join("total.json"),
        5.0,
        50.0,
    )
}

#[test]
fn new_fills_in_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    assert_eq!(config.listen, DEFAULT_LISTEN);
    assert_eq!(config.listen.to_string(), "127.0.0.1:0");
    assert_eq!(config.upstream, DEFAULT_UPSTREAM);
    assert!(!config.capture);
}

#[test]
fn config_debug_hides_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let debug = format!("{:?}", config(dir.path()));
    assert!(!debug.contains("sk-unit-secret"), "{debug}");
    assert!(debug.contains("[redacted]"));
}

#[test]
fn refuses_a_non_loopback_listen_address() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path());
    config.listen = "0.0.0.0:0".parse().unwrap();
    let error = Gateway::start(config).unwrap_err();
    assert!(matches!(error, Error::NonLoopback(_)), "{error}");
    assert!(!dir.path().join("total.lock").exists());
}

#[test]
fn reports_an_address_already_in_use() {
    let dir = tempfile::tempdir().unwrap();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut config = config(dir.path());
    config.listen = taken.local_addr().unwrap();
    let error = Gateway::start(config).unwrap_err();
    assert!(matches!(error, Error::Bind { .. }), "{error}");
    assert!(
        !dir.path().join("total.lock").exists(),
        "a failed start releases the ledger lock"
    );
}

#[test]
fn rejects_invalid_limits_before_listening() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path());
    config.run_limit_usd = 0.0;
    assert!(matches!(
        Gateway::start(config).unwrap_err(),
        Error::InvalidLimit(_)
    ));
}

#[test]
fn percent_encodes_tag_segments() {
    assert_eq!(encode_segment("score-1_a.b~"), "score-1_a.b~");
    assert_eq!(encode_segment("a/b c"), "a%2Fb%20c");
    assert_eq!(encode_segment("é"), "%C3%A9");
}

#[test]
fn remaining_is_the_tighter_cap_and_never_negative() {
    let spend = Spend {
        run_usd: 1.0,
        total_usd: 48.5,
        run_limit_usd: 5.0,
        total_limit_usd: 50.0,
    };
    assert!((spend.remaining_usd() - 1.5).abs() < 1e-12);
    let spend = Spend {
        run_usd: 6.0,
        ..spend
    };
    assert!(spend.remaining_usd().abs() < 1e-12);
}

#[test]
fn a_refused_or_silent_server_thread_fails_the_start_and_is_reaped() {
    let dir = tempfile::tempdir().unwrap();
    let mut gateway = Gateway::start(config(dir.path())).unwrap();
    let (ready, waiting) = std::sync::mpsc::sync_channel(1);
    refuse(&ready, Error::ServerStopped);
    let refused = wait_ready(&waiting, &mut gateway).unwrap_err();
    assert!(matches!(refused, Error::ServerStopped), "{refused}");
    assert!(gateway.thread.is_none(), "the server thread was joined");
    let (ready, waiting) = std::sync::mpsc::sync_channel(1);
    drop(ready);
    let silent = wait_ready(&waiting, &mut gateway).unwrap_err();
    assert!(matches!(silent, Error::ServerStopped), "{silent}");
    gateway.shutdown().unwrap();
}
