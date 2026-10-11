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

#[test]
fn reports_a_lock_folder_that_cannot_be_created_or_written() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a-file");
    std::fs::write(&file, "").unwrap();
    let error = LedgerLock::acquire(&file.join("deeper/ledger.json")).unwrap_err();
    assert!(matches!(error, Error::Io { .. }), "{error}");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let sealed = dir.path().join("sealed");
        std::fs::create_dir(&sealed).unwrap();
        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o500)).unwrap();
        let result = LedgerLock::acquire(&sealed.join("ledger.json"));
        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o700)).unwrap();
        // Root ignores directory permissions; everyone else is refused.
        if let Err(error) = result {
            assert!(matches!(error, Error::Io { .. }), "{error}");
        }
    }
}
