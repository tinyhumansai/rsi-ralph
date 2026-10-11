//! Bounded child-process execution shared by host verification and the script
//! loop.
//!
//! A child runs in its own process group with a cleared environment, stdin
//! closed, and stdout and stderr redirected to files in a caller-owned scratch
//! directory. The runner polls the two files and the child: when their combined
//! size passes the cap or the wall-clock timeout elapses, the whole process
//! group is killed. Nothing here decides whether a run *passed*; callers map the
//! [`Exit`] to their own verdicts.
use crate::{Error, Result};
use std::{
    ffi::OsStr,
    fs::File,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// One bounded invocation.
#[derive(Debug)]
pub struct ProcessSpec<'a> {
    /// Program to execute; resolved by the operating system.
    pub program: &'a OsStr,
    /// Arguments, passed verbatim.
    pub args: &'a [String],
    /// Working directory of the child.
    pub cwd: &'a Path,
    /// Directory receiving the `stdout` and `stderr` capture files.
    pub scratch: &'a Path,
    /// The complete child environment; nothing is inherited.
    pub env: &'a [(String, String)],
    /// Wall-clock limit.
    pub timeout: Duration,
    /// Limit on combined stdout and stderr bytes.
    pub cap: u64,
}

/// How a bounded invocation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// The child exited on its own.
    Exited {
        /// True for a zero exit status.
        success: bool,
        /// Exit code, absent when a signal ended the child.
        code: Option<i32>,
    },
    /// The wall-clock limit elapsed and the group was killed.
    TimedOut,
    /// Combined output passed the cap and the group was killed.
    OutputExceeded,
}

/// The result of a bounded invocation.
#[derive(Debug)]
pub struct ProcessOutcome {
    /// Termination kind.
    pub exit: Exit,
    /// Up to `cap + 1` bytes of stdout; empty unless the child exited.
    pub stdout: Vec<u8>,
    /// Up to `cap + 1` bytes of stderr; empty unless the child exited.
    pub stderr: Vec<u8>,
    /// Wall-clock milliseconds from spawn to termination.
    pub duration_ms: u64,
}

/// Runs one child to completion, timeout, or output overflow.
///
/// # Errors
///
/// Returns [`Error::HostFailed`] when the capture files cannot be created or
/// read, or the child cannot be spawned or polled.
pub fn run(spec: &ProcessSpec<'_>) -> Result<ProcessOutcome> {
    let stdout = spec.scratch.join("stdout");
    let stderr = spec.scratch.join("stderr");
    let out = File::create(&stdout).map_err(|_| Error::HostFailed)?;
    let err = File::create(&stderr).map_err(|_| Error::HostFailed)?;
    let mut invocation = Command::new(spec.program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        invocation.process_group(0);
    }
    let mut child = invocation
        .args(spec.args)
        .env_clear()
        .envs(spec.env.iter().map(|(k, v)| (k, v)))
        .current_dir(spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .spawn()
        .map_err(|_| Error::HostFailed)?;
    let start = Instant::now();
    let elapsed = |start: Instant| u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    loop {
        let bytes = std::fs::metadata(&stdout)
            .map_err(|_| Error::HostFailed)?
            .len()
            .saturating_add(
                std::fs::metadata(&stderr)
                    .map_err(|_| Error::HostFailed)?
                    .len(),
            );
        let exceeded = bytes > spec.cap;
        if exceeded || start.elapsed() >= spec.timeout {
            terminate(&mut child);
            let _ = child.wait();
            return Ok(ProcessOutcome {
                exit: if exceeded {
                    Exit::OutputExceeded
                } else {
                    Exit::TimedOut
                },
                stdout: vec![],
                stderr: vec![],
                duration_ms: elapsed(start),
            });
        }
        if let Some(status) = child.try_wait().map_err(|_| Error::HostFailed)? {
            terminate(&mut child);
            let limit = spec.cap.saturating_add(1);
            let mut out = Vec::new();
            File::open(&stdout)
                .and_then(|f| f.take(limit).read_to_end(&mut out))
                .map_err(|_| Error::HostFailed)?;
            let mut err = Vec::new();
            File::open(&stderr)
                .and_then(|f| f.take(limit).read_to_end(&mut err))
                .map_err(|_| Error::HostFailed)?;
            return Ok(ProcessOutcome {
                exit: Exit::Exited {
                    success: status.success(),
                    code: status.code(),
                },
                stdout: out,
                stderr: err,
                duration_ms: elapsed(start),
            });
        }
        thread::sleep(Duration::from_millis(2));
    }
}

/// Kills the child's whole process group, then the child itself.
fn terminate(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

#[cfg(test)]
mod test;
