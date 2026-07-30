use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use thiserror::Error;

use crate::executable::resolve_executable;
use crate::process::{BoundedOutput, ProcessError, ProcessLimits, run_bounded};

/// Configuration for an optional `cc -fsyntax-only` validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerConfig {
    /// Explicit compiler, or `None` to search `PATH` for `cc`.
    pub executable: Option<PathBuf>,
    /// Independent limits for version and validation calls.
    pub limits: ProcessLimits,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            executable: None,
            limits: ProcessLimits {
                timeout: std::time::Duration::from_secs(10),
                output_bytes: 2 * 1024 * 1024,
            },
        }
    }
}

/// Stable identity of the compiler command used for validation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompilerFingerprint {
    /// Normalized `cc --version` response.
    pub version_output: String,
    /// BLAKE3 digest of the response.
    pub digest: [u8; 32],
}

/// Result of one normal compiler invocation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompilerReport {
    /// Whether `cc` accepted the source.
    pub accepted: bool,
    /// Normal numeric exit status.
    pub exit_code: i32,
    /// Bounded standard output.
    pub stdout: String,
    /// Bounded standard error.
    pub stderr: String,
}

/// Operational failure distinct from a compiler rejection.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CompilerError {
    /// The compiler command was unavailable.
    #[error("C compiler is unavailable: {0}")]
    Unavailable(String),
    /// Version verification failed normally.
    #[error("C compiler version check failed (exit {exit_code:?}): {detail}")]
    VersionFailure {
        /// Numeric status when available.
        exit_code: Option<i32>,
        /// Bounded detail.
        detail: String,
    },
    /// The requested source basename was invalid.
    #[error("invalid in-memory compiler source name: {0}")]
    InvalidFileName(String),
    /// The temporary source could not be written.
    #[error("could not prepare the in-memory source for the compiler: {0}")]
    TemporarySource(String),
    /// The bounded child-process runner failed.
    #[error(transparent)]
    Process(#[from] ProcessError),
    /// The compiler was terminated by a signal or platform event.
    #[error("C compiler did not produce a normal exit status: {0}")]
    AbnormalExit(String),
}

/// Verified optional syntax-only C compiler.
#[derive(Clone, Debug)]
pub struct CompilerValidator {
    executable: PathBuf,
    fingerprint: CompilerFingerprint,
    limits: ProcessLimits,
}

impl CompilerValidator {
    /// Locates `cc`, verifies that `--version` succeeds, and fingerprints it.
    ///
    /// # Errors
    ///
    /// Returns [`CompilerError`] for discovery, process or version failures.
    pub fn locate(config: CompilerConfig) -> Result<Self, CompilerError> {
        let CompilerConfig { executable, limits } = config;
        limits.validate()?;
        let executable =
            resolve_executable(executable.as_deref(), "cc").map_err(CompilerError::Unavailable)?;
        let mut command = Command::new(&executable);
        command.arg("--version");
        configure_environment(&mut command);
        let output = run_bounded(&mut command, limits)?;
        if !output.success() {
            return Err(CompilerError::VersionFailure {
                exit_code: output.exit_code,
                detail: combined_output(&output),
            });
        }
        let version_output = combined_output(&output);
        if version_output.is_empty() {
            return Err(CompilerError::VersionFailure {
                exit_code: output.exit_code,
                detail: "the compiler produced no version response".to_owned(),
            });
        }
        Ok(Self {
            executable,
            fingerprint: CompilerFingerprint {
                digest: *blake3::hash(version_output.as_bytes()).as_bytes(),
                version_output,
            },
            limits,
        })
    }

    /// Returns the resolved compiler executable.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Returns the verified compiler fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> &CompilerFingerprint {
        &self.fingerprint
    }

    /// Runs `cc <argv> -fsyntax-only <basename>` without a shell.
    ///
    /// `argv` is treated as trusted, user-authorized compiler configuration:
    /// compiler plugins and similar flags can have side effects. Build recipes
    /// are never interpreted or executed. Relative include paths must be
    /// resolved by the caller because compilation occurs in an isolated
    /// directory.
    ///
    /// # Errors
    ///
    /// Returns [`CompilerError`] for operational failures. A normal nonzero
    /// compiler exit is returned as `CompilerReport { accepted: false, .. }`.
    pub fn validate(
        &self,
        requested_name: &Path,
        source: &str,
        argv: &[OsString],
    ) -> Result<CompilerReport, CompilerError> {
        let file_name = validated_basename(requested_name)?;
        let temporary =
            TempDir::new().map_err(|error| CompilerError::TemporarySource(error.to_string()))?;
        std::fs::write(temporary.path().join(&file_name), source.as_bytes())
            .map_err(|error| CompilerError::TemporarySource(error.to_string()))?;
        let mut command = Command::new(&self.executable);
        command
            .current_dir(temporary.path())
            .args(argv)
            .arg("-fsyntax-only")
            .arg(&file_name);
        configure_environment(&mut command);
        let output = run_bounded(&mut command, self.limits)?;
        let exit_code = output.exit_code.ok_or_else(|| {
            CompilerError::AbnormalExit(
                "the process was terminated without an exit code".to_owned(),
            )
        })?;
        Ok(CompilerReport {
            accepted: exit_code == 0,
            exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

fn validated_basename(requested: &Path) -> Result<String, CompilerError> {
    let file_name = requested
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            CompilerError::InvalidFileName(
                "the path must have a non-empty UTF-8 basename".to_owned(),
            )
        })?;
    if file_name.starts_with('-') || file_name.chars().any(char::is_control) {
        return Err(CompilerError::InvalidFileName(
            "the basename cannot start with '-' or contain control characters".to_owned(),
        ));
    }
    if !matches!(
        Path::new(file_name)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("c" | "h")
    ) {
        return Err(CompilerError::InvalidFileName(format!(
            "`{file_name}` must end in .c or .h"
        )));
    }
    Ok(file_name.to_owned())
}

fn configure_environment(command: &mut Command) {
    command
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("LANGUAGE", "en")
        .env("NO_COLOR", "1");
}

fn combined_output(output: &BoundedOutput) -> String {
    output
        .stdout
        .lines()
        .chain(output.stderr.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use tempfile::TempDir;

    use super::{CompilerConfig, CompilerError, CompilerValidator, ProcessError, ProcessLimits};

    #[cfg(unix)]
    fn executable_script(directory: &TempDir, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = directory.path().join("cc");
        std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n"))
            .expect("write fake compiler");
        let mut permissions = std::fs::metadata(&path)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("make script executable");
        path
    }

    #[cfg(unix)]
    fn compiler(script: PathBuf, timeout: Duration, cap: usize) -> CompilerValidator {
        let mut compiler = CompilerValidator::locate(CompilerConfig {
            executable: Some(script),
            limits: ProcessLimits {
                timeout: timeout.max(Duration::from_secs(2)),
                output_bytes: cap,
            },
        })
        .expect("verified fake compiler");
        compiler.limits = ProcessLimits {
            timeout,
            output_bytes: cap,
        };
        compiler
    }

    #[cfg(unix)]
    #[test]
    fn passes_argv_and_retains_basename_without_a_shell() {
        let directory = TempDir::new().expect("temporary script directory");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then echo "fake cc 1.0"; exit 0; fi
test "$1" = "-std=c99"
test "$2" = "-fsyntax-only"
test "$3" = "source.c"
test "$(cat "$3")" = "int source(void);"
"#,
        );
        let compiler = compiler(script, Duration::from_secs(5), 16 * 1024);

        let report = compiler
            .validate(
                Path::new("nested/source.c"),
                "int source(void);\n",
                &[OsString::from("-std=c99")],
            )
            .expect("normal compiler report");

        assert!(report.accepted);
        assert_eq!(report.exit_code, 0);
    }

    #[cfg(unix)]
    #[test]
    fn compiler_rejection_is_a_report_not_an_operational_error() {
        let directory = TempDir::new().expect("temporary script directory");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then echo "fake cc 1.0"; exit 0; fi
echo "source.c:1: error: expected declaration" >&2
exit 2
"#,
        );
        let compiler = compiler(script, Duration::from_secs(5), 16 * 1024);

        let report = compiler
            .validate(Path::new("source.c"), "broken", &[])
            .expect("normal rejection");

        assert!(!report.accepted);
        assert_eq!(report.exit_code, 2);
        assert!(report.stderr.contains("expected declaration"));
    }

    #[cfg(unix)]
    #[test]
    fn compiler_timeout_is_operational() {
        let directory = TempDir::new().expect("temporary script directory");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then echo "fake cc 1.0"; exit 0; fi
sleep 5
"#,
        );
        let compiler = compiler(script, Duration::from_millis(40), 16 * 1024);

        let error = compiler
            .validate(Path::new("source.c"), "int source(void);\n", &[])
            .expect_err("timeout");

        assert!(matches!(
            error,
            CompilerError::Process(ProcessError::Timeout { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn compiler_output_is_bounded() {
        let directory = TempDir::new().expect("temporary script directory");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then echo "fake cc 1.0"; exit 0; fi
i=0
while [ "$i" -lt 1000 ]; do
    echo "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" >&2
    i=$((i + 1))
done
"#,
        );
        let compiler = compiler(script, Duration::from_secs(2), 512);

        let error = compiler
            .validate(Path::new("source.c"), "int source(void);\n", &[])
            .expect_err("output cap");

        assert!(matches!(
            error,
            CompilerError::Process(ProcessError::OutputLimit { limit: 512 })
        ));
    }

    #[test]
    fn unavailable_explicit_compiler_is_operational() {
        let error = CompilerValidator::locate(CompilerConfig {
            executable: Some(PathBuf::from("/definitely/missing/cc")),
            ..CompilerConfig::default()
        })
        .expect_err("missing compiler");

        assert!(matches!(error, CompilerError::Unavailable(_)));
    }

    #[test]
    #[ignore = "requires a working system C compiler"]
    fn installed_compiler_smoke_test() {
        let compiler = CompilerValidator::locate(CompilerConfig::default()).expect("system cc");
        let report = compiler
            .validate(
                Path::new("smoke.c"),
                "int main(void) { return (0); }\n",
                &[OsString::from("-std=c99")],
            )
            .expect("compiler report");

        assert!(report.accepted, "{}", report.stderr);
    }
}
