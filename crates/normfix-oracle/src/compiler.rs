use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use thiserror::Error;

use crate::executable::resolve_executable;
use crate::norminette::strip_terminal_sequences;
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
    /// A real project source escaped the explicitly supplied project root.
    #[error("invalid project compiler source: {0}")]
    InvalidProjectSource(String),
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
            stdout: strip_terminal_sequences(&output.stdout),
            stderr: strip_terminal_sequences(&output.stderr),
        })
    }

    /// Runs a syntax-only compiler preflight against one real project source.
    ///
    /// Unlike [`Self::validate`], this method intentionally keeps the project
    /// directory as the compiler working directory. Relative quoted includes
    /// therefore resolve exactly as they do for a normal project invocation.
    /// The source and every path component are canonicalized first, and the
    /// source must remain a regular file inside `project_root`.
    ///
    /// `argv` is caller-authorized compiler configuration and is passed without
    /// a shell. This adapter never reads or executes Makefile recipes.
    ///
    /// # Errors
    ///
    /// Returns [`CompilerError`] for invalid paths or operational failures. A
    /// normal compiler rejection is returned as a report with `accepted=false`.
    pub fn validate_project_file(
        &self,
        project_root: &Path,
        source_path: &Path,
        argv: &[OsString],
    ) -> Result<CompilerReport, CompilerError> {
        let requested = if source_path.is_absolute() {
            source_path.to_path_buf()
        } else {
            project_root.join(source_path)
        };
        reject_project_symlink_components(project_root, &requested)?;
        let root = project_root.canonicalize().map_err(|error| {
            CompilerError::InvalidProjectSource(format!(
                "could not canonicalize project root `{}`: {error}",
                project_root.display()
            ))
        })?;
        let source = requested.canonicalize().map_err(|error| {
            CompilerError::InvalidProjectSource(format!(
                "could not canonicalize source `{}`: {error}",
                requested.display()
            ))
        })?;
        if !source.starts_with(&root) {
            return Err(CompilerError::InvalidProjectSource(format!(
                "source `{}` is outside `{}`",
                source.display(),
                root.display()
            )));
        }
        let metadata = std::fs::symlink_metadata(&source).map_err(|error| {
            CompilerError::InvalidProjectSource(format!(
                "could not inspect source `{}`: {error}",
                source.display()
            ))
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(CompilerError::InvalidProjectSource(format!(
                "source `{}` is not a regular non-symbolic file",
                source.display()
            )));
        }
        let relative = source.strip_prefix(&root).map_err(|error| {
            CompilerError::InvalidProjectSource(format!(
                "could not derive the project-relative source path: {error}"
            ))
        })?;
        let mut command = Command::new(&self.executable);
        command.current_dir(&root);
        add_project_validation_arguments(&mut command, argv);
        command.arg("--").arg(relative);
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
            stdout: strip_terminal_sequences(&output.stdout),
            stderr: strip_terminal_sequences(&output.stderr),
        })
    }
}

fn reject_project_symlink_components(root: &Path, source: &Path) -> Result<(), CompilerError> {
    let relative = source.strip_prefix(root).map_err(|error| {
        CompilerError::InvalidProjectSource(format!(
            "source `{}` is not lexically below `{}`: {error}",
            source.display(),
            root.display()
        ))
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(CompilerError::InvalidProjectSource(format!(
                "source `{}` contains a non-normal path component",
                source.display()
            )));
        }
        current.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&current).map_err(|error| {
            CompilerError::InvalidProjectSource(format!(
                "could not inspect source component `{}`: {error}",
                current.display()
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(CompilerError::InvalidProjectSource(format!(
                "source component `{}` is a symbolic link",
                current.display()
            )));
        }
    }
    Ok(())
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
        .env("GCC_COLORS", "")
        .env("CLICOLOR", "0")
        .env("CLICOLOR_FORCE", "0")
        .env("NO_COLOR", "1")
        // These variables can make GCC/Clang execute a different helper, add
        // implicit flags, or write dependency files even though this adapter
        // requested a read-only validation. Include paths needed by a project
        // are passed explicitly in `argv` by the caller.
        .env_remove("CCC_OVERRIDE_OPTIONS")
        .env_remove("COMPILER_PATH")
        .env_remove("DEPENDENCIES_OUTPUT")
        .env_remove("GCC_EXEC_PREFIX")
        .env_remove("GCC_SPECS")
        .env_remove("SUNPRO_DEPENDENCIES");
}

fn add_project_validation_arguments(command: &mut Command, argv: &[OsString]) {
    command.args(argv);
    let already_non_linking = argv
        .iter()
        .any(|argument| matches!(argument.to_str(), Some("-fsyntax-only" | "--analyze")));
    if !already_non_linking {
        command.arg("-fsyntax-only");
    }
}

fn combined_output(output: &BoundedOutput) -> String {
    let stdout = strip_terminal_sequences(&output.stdout);
    let stderr = strip_terminal_sequences(&output.stderr);
    stdout
        .lines()
        .chain(stderr.lines())
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

    use super::{
        CompilerConfig, CompilerError, CompilerValidator, ProcessError, ProcessLimits,
        add_project_validation_arguments,
    };

    /// The probe argument, answered by a guard ahead of every fake tool's body.
    const READINESS_PROBE: &str = "--normfix-readiness-probe";

    /// Writes a fake tool and does not return until the kernel will run it.
    ///
    /// Linux refuses to `execve` a file while any process holds it open for
    /// writing, and `cargo test` runs these on parallel threads that fork for
    /// their own subprocesses — so a sibling's child, between its `fork` and
    /// its `exec`, briefly counts as a writer for the script this thread just
    /// wrote. Waiting here gives every test a runnable tool instead of an
    /// occasional `Text file busy` that says nothing about normfix.
    ///
    /// This narrows the window rather than closing it: a sibling can fork again
    /// after the probe returns. The retry at the point of use below is the
    /// actual guarantee; the probe is what protects the call sites that do not
    /// have one, and it turns the common case into a wait instead of a failure.
    ///
    /// The probe exits before the body runs, so it cannot disturb a script
    /// that records what it was asked to do.
    #[cfg(unix)]
    fn executable_script(directory: &TempDir, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = directory.path().join("cc");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nset -eu\nif [ \"${{1:-}}\" = \"{READINESS_PROBE}\" ]; then exit 0; fi\n{body}\n"
            ),
        )
        .expect("write fake tool");
        let mut permissions = std::fs::metadata(&path)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("make script executable");
        for _ in 0..100 {
            match std::process::Command::new(&path)
                .arg(READINESS_PROBE)
                .output()
            {
                Ok(_) => return path,
                Err(error) if error.to_string().contains("Text file busy") => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("fake tool is not runnable: {error}"),
            }
        }
        panic!("fake tool was still reported as busy after retrying");
    }

    /// Retries while the kernel still reports a freshly written script as busy.
    ///
    /// Tests run in threads, and a child forked by another test inherits the
    /// write descriptor of a script this one just created until that child
    /// reaches its own `exec`. During that window `execve` fails with ETXTBSY.
    /// That is a harness race, not product behavior, so retry briefly rather
    /// than let the suite flake.
    #[cfg(unix)]
    fn retry_while_text_file_busy<T, E: std::fmt::Debug + std::fmt::Display>(
        what: &str,
        mut attempt: impl FnMut() -> Result<T, E>,
    ) -> T {
        for _ in 0..100 {
            match attempt() {
                Ok(value) => return value,
                Err(error) if error.to_string().contains("Text file busy") => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("{what}: {error:?}"),
            }
        }
        panic!("{what}: still reported as busy after retrying");
    }

    #[cfg(unix)]
    fn compiler(script: &Path, timeout: Duration, cap: usize) -> CompilerValidator {
        let mut compiler = retry_while_text_file_busy("verified fake compiler", || {
            CompilerValidator::locate(CompilerConfig {
                executable: Some(script.to_path_buf()),
                limits: ProcessLimits {
                    timeout: timeout.max(Duration::from_secs(2)),
                    output_bytes: cap,
                },
            })
        });
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
        let compiler = compiler(&script, Duration::from_secs(5), 16 * 1024);

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
    fn project_preflight_preserves_the_real_include_context() {
        let directory = TempDir::new().expect("temporary script directory");
        let project = TempDir::new().expect("temporary project");
        std::fs::create_dir(project.path().join("include")).expect("include directory");
        std::fs::create_dir(project.path().join("src")).expect("source directory");
        std::fs::write(
            project.path().join("include/project.h"),
            "#define VALUE 42\n",
        )
        .expect("header");
        std::fs::write(
            project.path().join("src/main.c"),
            "#include \"../include/project.h\"\nint value(void) { return (VALUE); }\n",
        )
        .expect("source");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then echo "fake cc 1.0"; exit 0; fi
test "$1" = "-Wall"
test "$2" = "-Wextra"
test "$3" = "-Werror"
test "$4" = "-fsyntax-only"
test "$5" = "--"
test "$6" = "src/main.c"
test -f "include/project.h"
grep 'project.h' "$6" >/dev/null
"#,
        );
        let compiler = compiler(&script, Duration::from_secs(5), 16 * 1024);

        let report = compiler
            .validate_project_file(
                project.path(),
                &project.path().join("src/main.c"),
                &[
                    OsString::from("-Wall"),
                    OsString::from("-Wextra"),
                    OsString::from("-Werror"),
                    OsString::from("-fsyntax-only"),
                ],
            )
            .expect("project preflight");

        assert!(report.accepted);
    }

    #[cfg(unix)]
    #[test]
    fn project_preflight_rejects_a_source_outside_the_root() {
        let directory = TempDir::new().expect("temporary script directory");
        let project = TempDir::new().expect("temporary project");
        let outside = TempDir::new().expect("outside directory");
        let outside_source = outside.path().join("outside.c");
        std::fs::write(&outside_source, "int outside(void);\n").expect("outside source");
        let script = executable_script(
            &directory,
            "if [ \"$1\" = \"--version\" ]; then echo 'fake cc 1.0'; exit 0; fi",
        );
        let compiler = compiler(&script, Duration::from_secs(5), 16 * 1024);

        let error = compiler
            .validate_project_file(project.path(), &outside_source, &[])
            .expect_err("outside source must fail closed");

        assert!(matches!(error, CompilerError::InvalidProjectSource(_)));
    }

    #[cfg(unix)]
    #[test]
    fn project_preflight_rejects_a_symlink_component() {
        use std::os::unix::fs::symlink;

        let directory = TempDir::new().expect("temporary script directory");
        let project = TempDir::new().expect("temporary project");
        let real = project.path().join("real");
        std::fs::create_dir(&real).expect("real directory");
        std::fs::write(real.join("source.c"), "int source(void);\n").expect("source");
        symlink(&real, project.path().join("linked")).expect("directory symlink");
        let script = executable_script(
            &directory,
            "if [ \"$1\" = \"--version\" ]; then echo 'fake cc 1.0'; exit 0; fi",
        );
        let compiler = compiler(&script, Duration::from_secs(5), 16 * 1024);

        let error = compiler
            .validate_project_file(project.path(), &project.path().join("linked/source.c"), &[])
            .expect_err("symlink traversal must fail closed");

        assert!(matches!(error, CompilerError::InvalidProjectSource(_)));
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
        let compiler = compiler(&script, Duration::from_secs(5), 16 * 1024);

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
        let compiler = compiler(&script, Duration::from_millis(40), 16 * 1024);

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
        let compiler = compiler(&script, Duration::from_secs(2), 512);

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
    fn project_validation_cannot_link_by_default() {
        let mut command = std::process::Command::new("cc");
        add_project_validation_arguments(&mut command, &[]);

        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![std::ffi::OsStr::new("-fsyntax-only")]
        );
    }

    #[test]
    fn clang_static_analysis_is_not_disabled_by_syntax_only_mode() {
        let mut command = std::process::Command::new("cc");
        add_project_validation_arguments(
            &mut command,
            &[OsString::from("--analyze"), OsString::from("-Wall")],
        );

        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![
                std::ffi::OsStr::new("--analyze"),
                std::ffi::OsStr::new("-Wall")
            ]
        );
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
