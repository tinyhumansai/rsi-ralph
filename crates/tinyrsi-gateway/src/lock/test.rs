//! Unit tests for the ledger lock.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::LedgerLock;
use crate::error::Error;

#[test]
fn excludes_a_second_owner_until_released() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger-total.json");
    let first = LedgerLock::acquire(&ledger).unwrap();
    assert!(dir.path().join("ledger-total.lock").exists());
    assert!(matches!(
        LedgerLock::acquire(&ledger).unwrap_err(),
        Error::Locked(_)
    ));
    drop(first);
    assert!(!dir.path().join("ledger-total.lock").exists());
    let _again = LedgerLock::acquire(&ledger).unwrap();
}

#[test]
fn reports_unusable_lock_paths() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a-file");
    std::fs::write(&file, "").unwrap();
    let error = LedgerLock::acquire(&file.join("ledger.json")).unwrap_err();
    assert!(matches!(error, Error::Io { .. }), "{error}");
}
