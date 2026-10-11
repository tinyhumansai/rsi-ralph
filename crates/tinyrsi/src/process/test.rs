//! Termination kinds and capture of the shared bounded process runner.
#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]
use super::*;

fn sh(script: &str, dir: &Path, timeout_ms: u64, cap: u64) -> ProcessOutcome {
    let args = vec!["-c".to_owned(), script.to_owned()];
    let env = vec![
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ("PROBE".to_owned(), "set".to_owned()),
    ];
    run(&ProcessSpec {
        program: OsStr::new("/bin/sh"),
        args: &args,
        cwd: dir,
        scratch: dir,
        env: &env,
        timeout: Duration::from_millis(timeout_ms),
        cap,
    })
    .unwrap()
}

#[test]
fn captures_stdout_and_stderr_separately_with_only_the_given_environment() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = sh(
        "echo \"$PROBE ${HOME:-nohome}\"; echo err >&2; pwd",
        dir.path(),
        10_000,
        1_000,
    );
    assert_eq!(
        outcome.exit,
        Exit::Exited {
            success: true,
            code: Some(0)
        }
    );
    let stdout = String::from_utf8(outcome.stdout).unwrap();
    assert!(stdout.starts_with("set nohome\n"));
    let cwd = std::fs::canonicalize(dir.path()).unwrap();
    assert!(stdout.trim_end().ends_with(cwd.file_name().unwrap().to_str().unwrap()));
    assert_eq!(outcome.stderr, b"err\n");
}

#[test]
fn reports_a_non_zero_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = sh("exit 3", dir.path(), 10_000, 1_000);
    assert_eq!(
        outcome.exit,
        Exit::Exited {
            success: false,
            code: Some(3)
        }
    );
}

#[test]
fn kills_a_child_that_outlives_the_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = sh("sleep 5", dir.path(), 50, 1_000);
    assert_eq!(outcome.exit, Exit::TimedOut);
    assert!(outcome.duration_ms < 5_000);
    assert!(outcome.stdout.is_empty());
}

#[test]
fn kills_a_child_that_exceeds_the_output_cap() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = sh("while true; do echo flood; done", dir.path(), 10_000, 64);
    assert_eq!(outcome.exit, Exit::OutputExceeded);
}

#[test]
fn fails_when_the_program_cannot_be_spawned() {
    let dir = tempfile::tempdir().unwrap();
    let result = run(&ProcessSpec {
        program: OsStr::new("/nonexistent/program"),
        args: &[],
        cwd: dir.path(),
        scratch: dir.path(),
        env: &[],
        timeout: Duration::from_secs(1),
        cap: 1,
    });
    assert_eq!(result.unwrap_err(), Error::HostFailed);
    let result = run(&ProcessSpec {
        program: OsStr::new("/bin/sh"),
        args: &[],
        cwd: dir.path(),
        scratch: &dir.path().join("missing"),
        env: &[],
        timeout: Duration::from_secs(1),
        cap: 1,
    });
    assert_eq!(result.unwrap_err(), Error::HostFailed);
}
