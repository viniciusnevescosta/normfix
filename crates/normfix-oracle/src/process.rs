use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
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
    run_bounded_inner(command, limits, None).map(|(output, _)| output)
}

/// Runs a process while counting a tool-owned log file against the same output
/// budget as stdout and stderr. The path should live in a private temporary
/// directory and need not exist before the child starts.
pub(crate) fn run_bounded_with_log_file(
    command: &mut Command,
    limits: ProcessLimits,
    log_path: &Path,
) -> Result<(BoundedOutput, String), ProcessError> {
    run_bounded_inner(command, limits, Some(log_path)).and_then(|(output, log)| {
        log.map_or_else(
            || {
                Err(ProcessError::CaptureRead(format!(
                    "external tool did not create `{}`",
                    log_path.display()
                )))
            },
            |log| Ok((output, log)),
        )
    })
}

fn run_bounded_inner(
    command: &mut Command,
    limits: ProcessLimits,
    log_path: Option<&Path>,
) -> Result<(BoundedOutput, Option<String>), ProcessError> {
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
    let mut containment = match Containment::around(&child) {
        Ok(containment) => containment,
        Err(error) => {
            terminate_and_reap(&mut child);
            return Err(error);
        }
    };
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
        let output_size = match combined_capture_len(&stdout, &stderr, log_path) {
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

    // A verified tool can still launch a helper and exit before it. The tool's
    // own successful status must not turn that helper into an unbounded orphan
    // or let it keep appending to the capture after the size check below.
    containment.terminate_remaining();
    let output_size = combined_capture_len(&stdout, &stderr, log_path)?;
    if output_size > u64::try_from(limits.output_bytes).unwrap_or(u64::MAX) {
        return Err(ProcessError::OutputLimit {
            limit: limits.output_bytes,
        });
    }
    let output = BoundedOutput {
        exit_code: status.code(),
        stdout: read_capture(&mut stdout, limits.output_bytes)?,
        stderr: read_capture(&mut stderr, limits.output_bytes)?,
    };
    let log = log_path
        .map(|path| {
            let mut file = open_regular_capture(path)?;
            read_capture(&mut file, limits.output_bytes)
        })
        .transpose()?;
    Ok((output, log))
}

fn capture_len(file: &File) -> Result<u64, ProcessError> {
    file.metadata()
        .map(|metadata| metadata.len())
        .map_err(|error| ProcessError::CaptureRead(error.to_string()))
}

fn combined_capture_len(
    stdout: &File,
    stderr: &File,
    log_path: Option<&Path>,
) -> Result<u64, ProcessError> {
    let captured = capture_len(stdout)?.saturating_add(capture_len(stderr)?);
    let log = log_path.map_or(Ok(0), |path| match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            Ok(metadata.len())
        }
        Ok(_) => Err(ProcessError::CaptureRead(format!(
            "external tool capture `{}` is not a regular file",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(ProcessError::CaptureRead(error.to_string())),
    })?;
    Ok(captured.saturating_add(log))
}

#[cfg(unix)]
fn open_regular_capture(path: &Path) -> Result<File, ProcessError> {
    use nix::fcntl::{OFlag, open};
    use nix::sys::stat::Mode;

    let descriptor = open(
        path,
        OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|error| ProcessError::CaptureRead(error.to_string()))?;
    let file = File::from(descriptor);
    if !file
        .metadata()
        .map_err(|error| ProcessError::CaptureRead(error.to_string()))?
        .is_file()
    {
        return Err(ProcessError::CaptureRead(format!(
            "external tool capture `{}` is not a regular file",
            path.display()
        )));
    }
    Ok(file)
}

#[cfg(not(unix))]
fn open_regular_capture(path: &Path) -> Result<File, ProcessError> {
    let file = File::open(path).map_err(|error| ProcessError::CaptureRead(error.to_string()))?;
    if !file
        .metadata()
        .map_err(|error| ProcessError::CaptureRead(error.to_string()))?
        .is_file()
    {
        return Err(ProcessError::CaptureRead(format!(
            "external tool capture `{}` is not a regular file",
            path.display()
        )));
    }
    Ok(file)
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
/// establishes the group before the program starts — `process_group(0)`
/// applies between fork and exec — and signals every ordinary descendant in
/// that group on the way out. A deliberately hostile program can create a new
/// session and leave a Unix process group; process groups are containment for
/// cooperative developer tools, not an operating-system sandbox. Windows has
/// no equivalent pre-start hook, so the child is placed in a job object
/// immediately after spawn, and the job kills everything in it as soon as its
/// last handle closes. That leaves a small window between spawn and assignment
/// and is why the compatibility policy states the two platforms separately
/// instead of claiming they are identical.
struct Containment {
    #[cfg(windows)]
    _job: Option<win32job::Job>,
    #[cfg(unix)]
    process_group: i32,
}

impl Containment {
    #[cfg(windows)]
    fn around(child: &std::process::Child) -> Result<Self, ProcessError> {
        use std::os::windows::io::AsRawHandle as _;

        let mut limits = win32job::ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        let job = win32job::Job::create_with_limit_info(&limits).map_err(|error| {
            ProcessError::Spawn(format!("could not create process containment: {error}"))
        })?;
        job.assign_process(child.as_raw_handle() as isize)
            .map_err(|error| {
                ProcessError::Spawn(format!("could not assign process containment: {error}"))
            })?;
        Ok(Self { _job: Some(job) })
    }

    #[cfg(unix)]
    fn around(child: &std::process::Child) -> Result<Self, ProcessError> {
        let process_group = i32::try_from(child.id()).map_err(|_| {
            ProcessError::Spawn(
                "child process id does not fit the platform process-group id".to_owned(),
            )
        })?;
        Ok(Self { process_group })
    }

    #[cfg(not(any(unix, windows)))]
    fn around(_child: &std::process::Child) -> Result<Self, ProcessError> {
        Ok(Self {})
    }

    fn terminate_remaining(&mut self) {
        #[cfg(unix)]
        signal_process_group(self.process_group);
        #[cfg(windows)]
        {
            // `KILL_ON_JOB_CLOSE` applies to every descendant still in the job.
            self._job.take();
        }
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
    if let Ok(pid) = i32::try_from(child.id()) {
        signal_process_group(pid);
    }
}

#[cfg(unix)]
fn signal_process_group(process_group: i32) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let _ = killpg(Pid::from_raw(process_group), Signal::SIGKILL);
}

/// On Windows the tree dies when the job object closes, which `Containment`
/// owns. Killing the direct child here would leave its descendants running.
#[cfg(not(unix))]
fn terminate_process_group(_child: &std::process::Child) {}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use std::fs;
    use std::process::Command;
    use std::thread;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::{
        ProcessError, ProcessLimits, open_regular_capture, run_bounded, run_bounded_with_log_file,
    };

    /// A tool that outlives its own deadline by spawning a helper.
    ///
    /// The helper records that it started, waits past the parent's deadline,
    /// and then records that it survived. Both marks matter: without the first
    /// the test could pass because the helper never ran at all, which would
    /// prove nothing about containment.
    fn escaping_tool(directory: &TempDir) -> Command {
        #[cfg(windows)]
        {
            let script = directory.path().join("escape.cmd");
            let body = concat!(
                "@echo off\r\n",
                // `ping` rather than `timeout`: this module gives every tool a null
                // stdin, and `timeout` refuses to run without a console.
                "start \"\" /b cmd /c \"echo x> started & ping -n 8 127.0.0.1 >nul & echo x> escaped\"\r\n",
                "ping -n 60 127.0.0.1 >nul\r\n",
            );
            fs::write(&script, body).expect("write escaping tool");
            let mut command = Command::new("cmd");
            command.arg("/c").arg(&script);
            command.current_dir(directory.path());
            command
        }
        #[cfg(unix)]
        {
            let mut command = Command::new("/bin/sh");
            command.arg("-c").arg(concat!(
                ": > started\n",
                "( sleep 7; : > escaped ) &\n",
                "sleep 60\n",
            ));
            command.current_dir(directory.path());
            command
        }
    }

    fn successful_tool_with_helper(directory: &TempDir) -> Command {
        #[cfg(windows)]
        {
            let script = directory.path().join("successful.cmd");
            let body = concat!(
                "@echo off\r\n",
                "start \"\" /b cmd /c \"echo x> started & ping -n 4 127.0.0.1 >nul & echo x> escaped\"\r\n",
                "exit /b 0\r\n",
            );
            fs::write(&script, body).expect("write successful tool");
            let mut command = Command::new("cmd");
            command.arg("/c").arg(&script);
            command.current_dir(directory.path());
            command
        }
        #[cfg(unix)]
        {
            let mut command = Command::new("/bin/sh");
            command.arg("-c").arg(concat!(
                ": > started\n",
                "( sleep 2; : > escaped ) &\n",
                "exit 0\n",
            ));
            command.current_dir(directory.path());
            command
        }
    }

    #[test]
    fn a_timeout_takes_the_whole_process_tree_with_it() {
        // An attempt only counts once the helper has recorded that it started.
        // Under a loaded machine the deadline can arrive first, and a run where
        // the helper never ran says nothing either way — so such a run is
        // retried rather than being allowed to pass or fail on timing.
        for _ in 1..=3 {
            let directory = TempDir::new().expect("temporary directory");
            let mut command = escaping_tool(&directory);

            let error = run_bounded(
                &mut command,
                ProcessLimits {
                    timeout: Duration::from_secs(2),
                    output_bytes: 1024,
                },
            )
            .expect_err("the tool sleeps far past its deadline");
            assert!(matches!(error, ProcessError::Timeout { .. }), "{error:?}");

            if !directory.path().join("started").exists() {
                continue;
            }

            // Long enough that a helper which was merely orphaned, rather than
            // killed, would have finished writing several times over.
            thread::sleep(Duration::from_secs(8));
            assert!(
                !directory.path().join("escaped").exists(),
                "a helper outlived the deadline of the tool that spawned it",
            );
            return;
        }
        panic!("the helper never started, so containment was never exercised");
    }

    #[test]
    fn a_successful_tool_cannot_leave_a_helper_running() {
        let directory = TempDir::new().expect("temporary directory");
        let mut command = successful_tool_with_helper(&directory);

        let output = run_bounded(
            &mut command,
            ProcessLimits {
                timeout: Duration::from_secs(5),
                output_bytes: 1024,
            },
        )
        .expect("the direct tool exits successfully");

        assert!(output.success());
        assert!(
            directory.path().join("started").exists(),
            "the helper never started, so containment was not exercised"
        );
        thread::sleep(Duration::from_secs(3));
        assert!(
            !directory.path().join("escaped").exists(),
            "a helper outlived a successful tool invocation"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_tool_owned_fifo_cannot_block_capture_reading() {
        use nix::sys::stat::Mode;
        use nix::unistd::mkfifo;

        let directory = TempDir::new().expect("temporary directory");
        let fifo = directory.path().join("capture");
        mkfifo(&fifo, Mode::S_IRUSR | Mode::S_IWUSR).expect("create fifo");

        let error = open_regular_capture(&fifo).expect_err("a FIFO is not a capture file");

        assert!(matches!(error, ProcessError::CaptureRead(_)));
    }

    #[cfg(unix)]
    #[test]
    fn a_tool_owned_log_counts_toward_the_output_limit() {
        let directory = TempDir::new().expect("temporary directory");
        let log = directory.path().join("tool.log");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("while :; do printf '%064d' 0 >> \"$1\"; done")
            .arg("normfix-test")
            .arg(&log);

        let error = run_bounded_with_log_file(
            &mut command,
            ProcessLimits {
                timeout: Duration::from_secs(5),
                output_bytes: 512,
            },
            &log,
        )
        .expect_err("the auxiliary log must be bounded");

        assert!(matches!(error, ProcessError::OutputLimit { limit: 512 }));
    }
}
