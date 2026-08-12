use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tempfile::tempfile;
use thiserror::Error;

/// Resource limits applied to every external tool invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessLimits {
    /// Maximum wall-clock duration.
    pub timeout: Duration,
    /// Maximum combined standard-output and standard-error size.
    pub output_bytes: usize,
}

impl ProcessLimits {
    /// Conservative defaults suitable for a single source file.
    #[must_use]
    pub const fn per_file_default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            output_bytes: 1024 * 1024,
        }
    }

    pub(crate) fn validate(self) -> Result<Self, ProcessError> {
        if self.timeout.is_zero() {
            return Err(ProcessError::InvalidLimits(
                "timeout must be greater than zero".to_owned(),
            ));
        }
        if self.output_bytes == 0 {
            return Err(ProcessError::InvalidLimits(
                "output limit must be greater than zero".to_owned(),
            ));
        }
        Ok(self)
    }
}

/// Captured, bounded process output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BoundedOutput {
    /// Child exit status as a platform-independent numeric code when available.
    pub exit_code: Option<i32>,
    /// Standard output decoded lossily as UTF-8.
    pub stdout: String,
    /// Standard error decoded lossily as UTF-8.
    pub stderr: String,
}

impl BoundedOutput {
    /// Returns whether the child exited successfully.
    #[must_use]
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// Operational failure while invoking an external process.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProcessError {
    /// Limits were internally inconsistent.
    #[error("invalid process limits: {0}")]
    InvalidLimits(String),
    /// Temporary capture storage could not be prepared.
    #[error("could not prepare bounded process output: {0}")]
    CaptureSetup(String),
    /// The executable could not be spawned.
    #[error("could not spawn external tool: {0}")]
    Spawn(String),
    /// Process status could not be inspected.
    #[error("could not wait for external tool: {0}")]
    Wait(String),
    /// Process did not finish before its deadline.
    #[error("external tool exceeded its {timeout_ms} ms timeout")]
    Timeout {
        /// Configured timeout in milliseconds.
        timeout_ms: u128,
    },
    /// Combined output exceeded the configured cap.
    #[error("external tool exceeded its {limit} byte output limit")]
    OutputLimit {
        /// Configured combined cap.
        limit: usize,
    },
    /// Output capture could not be read.
    #[error("could not read external tool output: {0}")]
    CaptureRead(String),
}

pub(crate) fn run_bounded(
    command: &mut Command,
    limits: ProcessLimits,
) -> Result<BoundedOutput, ProcessError> {
    let limits = limits.validate()?;
    let mut stdout = tempfile().map_err(|error| ProcessError::CaptureSetup(error.to_string()))?;
    let mut stderr = tempfile().map_err(|error| ProcessError::CaptureSetup(error.to_string()))?;
    let stdout_child = stdout
        .try_clone()
        .map_err(|error| ProcessError::CaptureSetup(error.to_string()))?;
    let stderr_child = stderr
        .try_clone()
        .map_err(|error| ProcessError::CaptureSetup(error.to_string()))?;

    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_child))
        .stderr(Stdio::from(stderr_child));
    configure_process_group(command);
    let mut child = command
        .spawn()
        .map_err(|error| ProcessError::Spawn(error.to_string()))?;
    // Held for the child's whole lifetime: on Windows the containment *is* this
    // value, and dropping it kills anything the tool left running.
    let _containment = Containment::around(&child);
    let started = Instant::now();

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                terminate_and_reap(&mut child);
                return Err(ProcessError::Wait(error.to_string()));
            }
        }
        let output_size = match combined_capture_len(&stdout, &stderr) {
            Ok(size) => size,
            Err(error) => {
                terminate_and_reap(&mut child);
                return Err(error);
            }
        };
        if output_size > u64::try_from(limits.output_bytes).unwrap_or(u64::MAX) {
            terminate_and_reap(&mut child);
            return Err(ProcessError::OutputLimit {
                limit: limits.output_bytes,
            });
        }
        if started.elapsed() >= limits.timeout {
            terminate_and_reap(&mut child);
            return Err(ProcessError::Timeout {
                timeout_ms: limits.timeout.as_millis(),
            });
        }
        thread::sleep(Duration::from_millis(5));
    };

    let output_size = combined_capture_len(&stdout, &stderr)?;
    if output_size > u64::try_from(limits.output_bytes).unwrap_or(u64::MAX) {
        return Err(ProcessError::OutputLimit {
            limit: limits.output_bytes,
        });
    }
    Ok(BoundedOutput {
        exit_code: status.code(),
        stdout: read_capture(&mut stdout, limits.output_bytes)?,
        stderr: read_capture(&mut stderr, limits.output_bytes)?,
    })
}

fn capture_len(file: &File) -> Result<u64, ProcessError> {
    file.metadata()
        .map(|metadata| metadata.len())
        .map_err(|error| ProcessError::CaptureRead(error.to_string()))
}

fn combined_capture_len(stdout: &File, stderr: &File) -> Result<u64, ProcessError> {
    Ok(capture_len(stdout)?.saturating_add(capture_len(stderr)?))
}

fn read_capture(file: &mut File, limit: usize) -> Result<String, ProcessError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| ProcessError::CaptureRead(error.to_string()))?;
    let mut bytes = Vec::new();
    file.take(u64::try_from(limit).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|error| ProcessError::CaptureRead(error.to_string()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn terminate_and_reap(child: &mut std::process::Child) {
    terminate_process_group(child);
    let _ = child.kill();
    let _ = child.wait();
}

/// Keeps a tool's whole process tree bounded, not just the process spawned.
///
/// A checker that spawns helpers must not be able to leave them running after
/// its own deadline, or the bound this module exists to enforce is only a bound
/// on one process.
///
/// The two platforms reach that guarantee from opposite directions. Unix
/// establishes it before the program starts — `process_group(0)` applies
/// between fork and exec, so no descendant can ever escape — and signals the
/// group on the way out. Windows has no equivalent pre-start hook, so the child
/// is placed in a job object immediately after spawn, and the job kills
/// everything in it as soon as its last handle closes. That leaves a window
/// between spawn and assignment in which a grandchild could break away; it is
/// microseconds wide and it is the reason the compatibility policy states the
/// two platforms separately instead of claiming they are identical.
struct Containment {
    #[cfg(windows)]
    _job: Option<win32job::Job>,
}

impl Containment {
    #[cfg(windows)]
    fn around(child: &std::process::Child) -> Self {
        use std::os::windows::io::AsRawHandle as _;

        let mut limits = win32job::ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        let job = win32job::Job::create_with_limit_info(&limits)
            .ok()
            .filter(|job| job.assign_process(child.as_raw_handle() as isize).is_ok());
        Self { _job: job }
    }

    #[cfg(not(windows))]
    fn around(_child: &std::process::Child) -> Self {
        Self {}
    }
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_process_group(child: &std::process::Child) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    if let Ok(pid) = i32::try_from(child.id()) {
        let _ = killpg(Pid::from_raw(pid), Signal::SIGKILL);
    }
}

/// On Windows the tree dies when the job object closes, which `Containment`
/// owns. Killing the direct child here would leave its descendants running.
#[cfg(not(unix))]
fn terminate_process_group(_child: &std::process::Child) {}
