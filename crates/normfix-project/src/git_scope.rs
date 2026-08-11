//! Read-only discovery of paths selected by Git working-tree state.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Git state used to select candidate project paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitScope {
    /// Tracked working-tree changes plus untracked, non-ignored files.
    Changed,
    /// Changes currently recorded in the index.
    Staged,
}

/// Resource and executable settings for Git scope resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitScopeOptions {
    /// Maximum duration of each Git invocation.
    pub timeout: Duration,
    /// Maximum combined standard-output and standard-error bytes per invocation.
    pub output_bytes: usize,
    /// Git executable to invoke directly, without a shell.
    pub git_executable: PathBuf,
}

impl Default for GitScopeOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            output_bytes: DEFAULT_OUTPUT_BYTES,
            git_executable: PathBuf::from("git"),
        }
    }
}

/// Failure while resolving a Git-backed path scope.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GitScopeError {
    /// The configured process limits cannot safely run Git.
    #[error("invalid Git process limits: {0}")]
    InvalidLimits(String),
    /// The requested root could not be inspected.
    #[error("could not inspect Git scope root `{root}`: {message}")]
    RootMetadata {
        /// Requested scope root.
        root: PathBuf,
        /// Operating-system error text.
        message: String,
    },
    /// The requested root is not a directory.
    #[error("Git scope root `{root}` is not a directory")]
    RootNotDirectory {
        /// Requested scope root.
        root: PathBuf,
    },
    /// The requested root traverses a symbolic link.
    #[error("Git scope root `{root}` traverses symbolic link `{component}`")]
    RootSymlink {
        /// Requested scope root.
        root: PathBuf,
        /// First forbidden symbolic-link component.
        component: PathBuf,
    },
    /// Git could not be started.
    #[error("could not start Git for {operation}: {message}")]
    Spawn {
        /// Git operation being performed.
        operation: &'static str,
        /// Operating-system error text.
        message: String,
    },
    /// Git status could not be observed.
    #[error("could not wait for Git during {operation}: {message}")]
    Wait {
        /// Git operation being performed.
        operation: &'static str,
        /// Operating-system error text.
        message: String,
    },
    /// Git did not finish before its deadline.
    #[error("Git {operation} exceeded its {timeout_ms} ms timeout")]
    Timeout {
        /// Git operation being performed.
        operation: &'static str,
        /// Configured timeout in milliseconds.
        timeout_ms: u128,
    },
    /// Git emitted more output than allowed.
    #[error("Git {operation} exceeded its {limit} byte output limit")]
    OutputLimit {
        /// Git operation being performed.
        operation: &'static str,
        /// Configured combined output cap.
        limit: usize,
    },
    /// Capturing Git output failed.
    #[error("could not capture Git output during {operation}: {message}")]
    Capture {
        /// Git operation being performed.
        operation: &'static str,
        /// I/O or worker error text.
        message: String,
    },
    /// Git reported an operational error.
    #[error("Git {operation} failed with status {status:?}: {stderr}")]
    GitFailure {
        /// Git operation being performed.
        operation: &'static str,
        /// Platform-independent exit code, when available.
        status: Option<i32>,
        /// Bounded standard error, decoded lossily for diagnostics.
        stderr: String,
    },
    /// NUL-delimited output was malformed or not representable as a path.
    #[error("Git returned invalid path output during {operation}: {message}")]
    InvalidOutput {
        /// Git operation being performed.
        operation: &'static str,
        /// Validation failure.
        message: String,
    },
    /// Git returned an absolute or escaping path.
    #[error("Git returned unsafe path `{path}` during {operation}")]
    UnsafePath {
        /// Git operation being performed.
        operation: &'static str,
        /// Untrusted path from Git.
        path: PathBuf,
    },
    /// A selected path changed or became unreadable during validation.
    #[error("could not validate Git-selected path `{path}`: {message}")]
    CandidateMetadata {
        /// Candidate absolute path.
        path: PathBuf,
        /// Operating-system error text.
        message: String,
    },
}

/// Resolves paths selected by `scope` below `root`.
///
/// Returned paths are absolute, lexically normalized, deduplicated and sorted.
/// Symbolic links and non-files are omitted. File-kind classification is left
/// to the caller so this API remains independent of a particular workflow.
/// Any Git, parsing or path-validation error fails the complete operation.
///
/// # Errors
///
/// Returns [`GitScopeError`] when the root or limits are invalid, Git cannot be
/// run safely, Git fails, or its output cannot be validated as confined paths.
pub fn resolve_git_scope(
    root: &Path,
    scope: GitScope,
    options: &GitScopeOptions,
) -> Result<Vec<PathBuf>, GitScopeError> {
    validate_options(options)?;
    let root = validated_root(root)?;
    let commands: &[(&str, &[&str])] = match scope {
        GitScope::Changed => &[
            (
                "working-tree diff",
                &[
                    "diff",
                    "--relative",
                    "--name-only",
                    "--diff-filter=ACMR",
                    "-z",
                    "--",
                    ".",
                ],
            ),
            (
                "untracked files",
                &[
                    "ls-files",
                    "--others",
                    "--exclude-standard",
                    "-z",
                    "--",
                    ".",
                ],
            ),
        ],
        GitScope::Staged => &[(
            "staged diff",
            &[
                "diff",
                "--relative",
                "--cached",
                "--name-only",
                "--diff-filter=ACMR",
                "-z",
                "--",
                ".",
            ],
        )],
    };

    let mut candidates = BTreeSet::new();
    for (operation, arguments) in commands {
        let output = run_git(&root, operation, arguments, options)?;
        for relative in parse_nul_paths(&output, operation)? {
            let candidate = validated_candidate(&root, &relative, operation)?;
            if let Some(candidate) = candidate {
                candidates.insert(candidate);
            }
        }
    }
    Ok(candidates.into_iter().collect())
}

fn validate_options(options: &GitScopeOptions) -> Result<(), GitScopeError> {
    if options.timeout.is_zero() {
        return Err(GitScopeError::InvalidLimits(
            "timeout must be greater than zero".to_owned(),
        ));
    }
    if options.output_bytes == 0 {
        return Err(GitScopeError::InvalidLimits(
            "output limit must be greater than zero".to_owned(),
        ));
    }
    if options.git_executable.as_os_str().is_empty() {
        return Err(GitScopeError::InvalidLimits(
            "Git executable must not be empty".to_owned(),
        ));
    }
    Ok(())
}

fn validated_root(root: &Path) -> Result<PathBuf, GitScopeError> {
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| GitScopeError::RootMetadata {
                root: root.to_path_buf(),
                message: error.to_string(),
            })?
            .join(root)
    };
    if let Some(component) =
        super::first_symlink_component(&absolute).map_err(|error| GitScopeError::RootMetadata {
            root: absolute.clone(),
            message: error.to_string(),
        })?
    {
        return Err(GitScopeError::RootSymlink {
            root: absolute,
            component,
        });
    }
    let metadata =
        fs::symlink_metadata(&absolute).map_err(|error| GitScopeError::RootMetadata {
            root: absolute.clone(),
            message: error.to_string(),
        })?;
    if !metadata.is_dir() {
        return Err(GitScopeError::RootNotDirectory { root: absolute });
    }
    fs::canonicalize(&absolute).map_err(|error| GitScopeError::RootMetadata {
        root: absolute.clone(),
        message: error.to_string(),
    })?;
    Ok(super::lexical_normalize(&absolute))
}

fn validated_candidate(
    root: &Path,
    relative: &Path,
    operation: &'static str,
) -> Result<Option<PathBuf>, GitScopeError> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(GitScopeError::UnsafePath {
            operation,
            path: relative.to_path_buf(),
        });
    }
    let candidate = root.join(relative);
    if !candidate.starts_with(root) {
        return Err(GitScopeError::UnsafePath {
            operation,
            path: relative.to_path_buf(),
        });
    }
    if super::first_symlink_component(&candidate)
        .map_err(|error| GitScopeError::CandidateMetadata {
            path: candidate.clone(),
            message: error.to_string(),
        })?
        .is_some()
    {
        return Ok(None);
    }
    let metadata =
        fs::symlink_metadata(&candidate).map_err(|error| GitScopeError::CandidateMetadata {
            path: candidate.clone(),
            message: error.to_string(),
        })?;
    Ok(metadata.is_file().then_some(candidate))
}

fn parse_nul_paths(output: &[u8], operation: &'static str) -> Result<Vec<PathBuf>, GitScopeError> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    if output.last() != Some(&0) {
        return Err(GitScopeError::InvalidOutput {
            operation,
            message: "missing final NUL separator".to_owned(),
        });
    }
    output[..output.len() - 1]
        .split(|byte| *byte == 0)
        .map(|bytes| {
            if bytes.is_empty() {
                return Err(GitScopeError::InvalidOutput {
                    operation,
                    message: "empty path record".to_owned(),
                });
            }
            #[cfg(unix)]
            {
                Ok(PathBuf::from(bytes_to_os_string(bytes)))
            }
            #[cfg(not(unix))]
            {
                bytes_to_os_string(bytes, operation).map(PathBuf::from)
            }
        })
        .collect()
}

#[cfg(unix)]
fn bytes_to_os_string(bytes: &[u8]) -> OsString {
    use std::os::unix::ffi::OsStringExt;

    OsString::from_vec(bytes.to_vec())
}

#[cfg(not(unix))]
fn bytes_to_os_string(bytes: &[u8], operation: &'static str) -> Result<OsString, GitScopeError> {
    String::from_utf8(bytes.to_vec())
        .map(OsString::from)
        .map_err(|error| GitScopeError::InvalidOutput {
            operation,
            message: error.to_string(),
        })
}

struct GitOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_git(
    root: &Path,
    operation: &'static str,
    arguments: &[&str],
    options: &GitScopeOptions,
) -> Result<Vec<u8>, GitScopeError> {
    let mut command = Command::new(&options.git_executable);
    command
        .args(arguments)
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| GitScopeError::Spawn {
        operation,
        message: error.to_string(),
    })?;
    let stdout = child.stdout.take().ok_or_else(|| GitScopeError::Capture {
        operation,
        message: "standard output pipe was unavailable".to_owned(),
    })?;
    let stderr = child.stderr.take().ok_or_else(|| GitScopeError::Capture {
        operation,
        message: "standard error pipe was unavailable".to_owned(),
    })?;
    let total = Arc::new(AtomicUsize::new(0));
    let stdout_worker = capture_worker(stdout, Arc::clone(&total), options.output_bytes);
    let stderr_worker = capture_worker(stderr, Arc::clone(&total), options.output_bytes);
    let started = Instant::now();

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                terminate_and_reap(&mut child);
                join_capture(stdout_worker, operation)?;
                join_capture(stderr_worker, operation)?;
                return Err(GitScopeError::Wait {
                    operation,
                    message: error.to_string(),
                });
            }
        }
        if total.load(Ordering::Relaxed) > options.output_bytes {
            terminate_and_reap(&mut child);
            join_capture(stdout_worker, operation)?;
            join_capture(stderr_worker, operation)?;
            return Err(GitScopeError::OutputLimit {
                operation,
                limit: options.output_bytes,
            });
        }
        if started.elapsed() >= options.timeout {
            terminate_and_reap(&mut child);
            join_capture(stdout_worker, operation)?;
            join_capture(stderr_worker, operation)?;
            return Err(GitScopeError::Timeout {
                operation,
                timeout_ms: options.timeout.as_millis(),
            });
        }
        thread::sleep(POLL_INTERVAL);
    };

    let captured = GitOutput {
        status,
        stdout: join_capture(stdout_worker, operation)?,
        stderr: join_capture(stderr_worker, operation)?,
    };
    if total.load(Ordering::Relaxed) > options.output_bytes {
        return Err(GitScopeError::OutputLimit {
            operation,
            limit: options.output_bytes,
        });
    }
    if !captured.status.success() {
        return Err(GitScopeError::GitFailure {
            operation,
            status: captured.status.code(),
            stderr: String::from_utf8_lossy(&captured.stderr).into_owned(),
        });
    }
    Ok(captured.stdout)
}

fn capture_worker<R: Read + Send + 'static>(
    mut reader: R,
    total: Arc<AtomicUsize>,
    limit: usize,
) -> thread::JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut captured = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            let previous = total.fetch_add(read, Ordering::Relaxed);
            let remaining = limit.saturating_sub(previous);
            captured.extend_from_slice(&buffer[..read.min(remaining)]);
        }
        Ok(captured)
    })
}

fn join_capture(
    worker: thread::JoinHandle<io::Result<Vec<u8>>>,
    operation: &'static str,
) -> Result<Vec<u8>, GitScopeError> {
    worker
        .join()
        .map_err(|_| GitScopeError::Capture {
            operation,
            message: "output capture worker panicked".to_owned(),
        })?
        .map_err(|error| GitScopeError::Capture {
            operation,
            message: error.to_string(),
        })
}

fn terminate_and_reap(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::{GitScope, GitScopeError, GitScopeOptions, resolve_git_scope};

    fn git(root: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .args(arguments)
            .current_dir(root)
            .status()
            .expect("run Git fixture command");
        assert!(status.success());
    }

    fn repository() -> TempDir {
        let temporary = TempDir::new().expect("temporary directory");
        git(temporary.path(), &["init", "--quiet"]);
        temporary
    }

    #[test]
    fn changed_includes_tracked_worktree_changes_and_untracked_files() {
        let repository = repository();
        let root = repository.path();
        fs::write(root.join("tracked file.c"), "before\n").expect("tracked file");
        fs::write(root.join("staged.h"), "staged\n").expect("staged file");
        git(root, &["add", "tracked file.c", "staged.h"]);
        fs::write(root.join("tracked file.c"), "after\n").expect("modified tracked file");
        fs::write(root.join("untracked file.h"), "new\n").expect("untracked file");

        let paths = resolve_git_scope(root, GitScope::Changed, &GitScopeOptions::default())
            .expect("changed scope");

        assert_eq!(
            paths,
            vec![root.join("tracked file.c"), root.join("untracked file.h")]
        );
    }

    #[test]
    fn staged_includes_only_index_changes_and_preserves_spaces() {
        let repository = repository();
        let root = repository.path();
        fs::write(root.join("staged file.c"), "staged\n").expect("staged file");
        fs::write(root.join("untracked.h"), "new\n").expect("untracked file");
        git(root, &["add", "staged file.c"]);

        let paths = resolve_git_scope(root, GitScope::Staged, &GitScopeOptions::default())
            .expect("staged scope");

        assert_eq!(paths, vec![root.join("staged file.c")]);
    }

    #[test]
    fn scopes_are_confined_to_a_subdirectory_root() {
        let repository = repository();
        let root = repository.path();
        fs::create_dir(root.join("project")).expect("project directory");
        fs::write(root.join("outside.c"), "outside\n").expect("outside file");
        fs::write(root.join("project/inside.c"), "inside\n").expect("inside file");
        git(root, &["add", "outside.c", "project/inside.c"]);

        let paths = resolve_git_scope(
            &root.join("project"),
            GitScope::Staged,
            &GitScopeOptions::default(),
        )
        .expect("subdirectory scope");

        assert_eq!(paths, vec![root.join("project/inside.c")]);
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_candidates_are_omitted() {
        use std::os::unix::fs::symlink;

        let repository = repository();
        let root = repository.path();
        let outside = root
            .parent()
            .expect("temporary parent")
            .join("outside-target");
        fs::write(&outside, "outside\n").expect("outside target");
        symlink(&outside, root.join("link.c")).expect("symbolic link");

        let paths = resolve_git_scope(root, GitScope::Changed, &GitScopeOptions::default())
            .expect("changed scope");

        assert!(paths.is_empty());
        fs::remove_file(outside).expect("remove outside target");
    }

    #[test]
    fn non_repository_is_a_typed_failure() {
        let temporary = TempDir::new().expect("temporary directory");

        let error = resolve_git_scope(
            temporary.path(),
            GitScope::Staged,
            &GitScopeOptions::default(),
        )
        .expect_err("non-repository must fail closed");

        assert!(matches!(error, GitScopeError::GitFailure { .. }));
    }

    #[test]
    fn output_limit_is_enforced() {
        let repository = repository();
        let root = repository.path();
        fs::write(root.join("long-name.c"), "staged\n").expect("staged file");
        git(root, &["add", "long-name.c"]);
        let options = GitScopeOptions {
            output_bytes: 2,
            ..GitScopeOptions::default()
        };

        let error = resolve_git_scope(root, GitScope::Staged, &options)
            .expect_err("output cap must fail closed");

        assert!(matches!(error, GitScopeError::OutputLimit { limit: 2, .. }));
    }

    #[cfg(unix)]
    #[test]
    fn timeout_is_enforced() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = TempDir::new().expect("temporary directory");
        let executable = temporary.path().join("slow-git");
        fs::write(&executable, "#!/bin/sh\nexec sleep 2\n").expect("fake Git executable");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
            .expect("executable permissions");
        let options = GitScopeOptions {
            timeout: Duration::from_millis(200),
            git_executable: executable,
            ..GitScopeOptions::default()
        };

        // The same ETXTBSY window the oracle tests already wait out: a child
        // forked by another test thread still holds a write descriptor to this
        // script until it reaches its own exec, and Linux will not execute a
        // file while a writer exists. That surfaces as a spawn failure rather
        // than the timeout this test is about.
        let error = (0..100)
            .find_map(
                |_| match resolve_git_scope(temporary.path(), GitScope::Staged, &options) {
                    Err(GitScopeError::Spawn { ref message, .. })
                        if message.contains("Text file busy") =>
                    {
                        std::thread::sleep(Duration::from_millis(20));
                        None
                    }
                    outcome => Some(outcome),
                },
            )
            .expect("the fake Git executable never became runnable")
            .expect_err("timeout must fail closed");

        assert!(matches!(error, GitScopeError::Timeout { .. }));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_file_names_round_trip_through_nul_output() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let repository = repository();
        let root = repository.path();
        let name = OsString::from_vec(b"invalid-\xff.c".to_vec());
        let path = root.join(std::path::PathBuf::from(name));
        fs::write(&path, "new\n").expect("non-UTF-8 file");

        let paths = resolve_git_scope(root, GitScope::Changed, &GitScopeOptions::default())
            .expect("changed scope");

        assert_eq!(paths, vec![path]);
    }
}
