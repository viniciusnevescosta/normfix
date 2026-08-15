//! Optional `clang-tidy` lens over C sources.
//!
//! This backend is a lens and nothing more. It is never required: a machine
//! without `clang-tidy` runs exactly as before. It never authorizes an edit,
//! because what it reports is a judgement about a program's behaviour rather
//! than a fact about its text, and normfix only edits on the second kind. What
//! it adds is a reading of ownership and control flow — a leak on a path
//! nobody took, a pointer used after it was released — which the parser cannot
//! reach and which is what fails a 42 defense.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use crate::executable::resolve_executable;
use crate::norminette::strip_terminal_sequences;
use crate::process::{ProcessError, ProcessLimits, run_bounded};

/// The checks this lens asks for.
///
/// Only the two families that answer questions about the program: the static
/// analyzer, which is where leaks and use-after-free live, and the bug-prone
/// checks. Everything else `clang-tidy` ships is house style for C++ — it
/// would argue with the Norm, and the Norm is the authority here.
const CHECKS: &str = "clang-analyzer-*,bugprone-*";

/// Configuration for the optional `clang-tidy` lens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClangTidyConfig {
    /// Explicit executable, or `None` to search `PATH`.
    pub executable: Option<PathBuf>,
    /// Bounds on the version and analysis calls.
    pub limits: ProcessLimits,
}

impl Default for ClangTidyConfig {
    fn default() -> Self {
        Self {
            executable: None,
            limits: ProcessLimits {
                timeout: std::time::Duration::from_secs(30),
                output_bytes: 2 * 1024 * 1024,
            },
        }
    }
}

/// Why the lens could not be opened.
#[derive(Debug, Error)]
pub enum ClangTidyError {
    /// No `clang-tidy` was found, which is not an error for a caller that
    /// treats the lens as optional.
    #[error("clang-tidy is unavailable: {0}")]
    Unavailable(String),
    /// The executable did not answer `--version`.
    #[error("clang-tidy did not report a version (exit code {exit_code:?}): {detail}")]
    VersionFailure {
        /// Exit status of the version call, absent when a signal ended it.
        exit_code: Option<i32>,
        /// Combined output, for the reader.
        detail: String,
    },
    /// The process could not be run within its bounds.
    #[error(transparent)]
    Process(#[from] ProcessError),
}

/// One finding, as `clang-tidy` reported it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClangTidyFinding {
    /// The check that produced it, such as `clang-analyzer-unix.Malloc`.
    pub check: String,
    /// One-line description.
    pub message: String,
    /// File the finding points at, as the tool printed it.
    pub path: String,
    /// One-based line.
    pub line: u32,
    /// One-based column.
    pub column: u32,
}

/// A verified, optional `clang-tidy`.
#[derive(Clone, Debug)]
pub struct ClangTidy {
    executable: PathBuf,
    version_output: String,
    limits: ProcessLimits,
}

impl ClangTidy {
    /// Locates `clang-tidy` and verifies it by its own version banner.
    ///
    /// # Errors
    ///
    /// Returns [`ClangTidyError`] when the executable is absent, cannot run, or
    /// answers `--version` with nothing. A caller treating the lens as optional
    /// should discard the error rather than fail the run.
    pub fn locate(config: ClangTidyConfig) -> Result<Self, ClangTidyError> {
        let ClangTidyConfig { executable, limits } = config;
        limits.validate()?;
        let executable = resolve_executable(executable.as_deref(), "clang-tidy")
            .map_err(ClangTidyError::Unavailable)?;
        let mut command = Command::new(&executable);
        command.arg("--version");
        let output = run_bounded(&mut command, limits)?;
        let version_output = strip_terminal_sequences(&output.stdout).trim().to_owned();
        if !output.success() || version_output.is_empty() {
            return Err(ClangTidyError::VersionFailure {
                exit_code: output.exit_code,
                detail: if version_output.is_empty() {
                    "the executable produced no version response".to_owned()
                } else {
                    version_output
                },
            });
        }
        Ok(Self {
            executable,
            version_output,
            limits,
        })
    }

    /// Returns the resolved executable.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Returns the version banner this lens was verified by.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version_output
    }

    /// Reads `path`, returning what the lens saw.
    ///
    /// The arguments after `--` are what a compiler would have received. With
    /// no compilation database, `clang-tidy` needs them to resolve the
    /// project's own headers; without them it reports missing includes rather
    /// than anything about the program.
    ///
    /// # Errors
    ///
    /// Returns [`ClangTidyError`] when the process exceeds its bounds. A
    /// non-zero exit is not an error: `clang-tidy` exits non-zero whenever it
    /// found something.
    pub fn analyze(
        &self,
        path: &Path,
        include_directories: &[PathBuf],
    ) -> Result<Vec<ClangTidyFinding>, ClangTidyError> {
        let mut command = Command::new(&self.executable);
        command.arg(format!("--checks={CHECKS}"));
        command.arg("--quiet");
        command.arg(path);
        command.arg("--");
        for directory in include_directories {
            let mut flag = OsString::from("-I");
            flag.push(directory);
            command.arg(flag);
        }
        let output = run_bounded(&mut command, self.limits)?;
        Ok(parse_findings(&strip_terminal_sequences(&output.stdout)))
    }
}

/// Reads the findings out of a `clang-tidy` report.
///
/// The tool prints `path:line:column: warning: message [check]` for a finding
/// and `note:` lines for the path that led to it. Only the findings are taken:
/// a note has no check name of its own and repeating the trace would bury the
/// one line the reader has to act on.
fn parse_findings(output: &str) -> Vec<ClangTidyFinding> {
    let mut findings = Vec::new();
    for line in output.lines() {
        let Some(finding) = parse_finding(line) else {
            continue;
        };
        findings.push(finding);
    }
    findings
}

fn parse_finding(line: &str) -> Option<ClangTidyFinding> {
    let (message, check) = line.rsplit_once(" [")?;
    let check = check.strip_suffix(']')?;
    // A check name is the tool's own identifier; anything else on the line is
    // source text that happens to end in a bracket.
    if check.is_empty()
        || !check
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-._,".contains(character))
    {
        return None;
    }
    let (location, message) = message.split_once(": warning: ")?;
    let (rest, column) = location.rsplit_once(':')?;
    let (path, line_number) = rest.rsplit_once(':')?;
    Some(ClangTidyFinding {
        check: check.to_owned(),
        message: message.trim().to_owned(),
        path: path.to_owned(),
        line: line_number.parse().ok()?,
        column: column.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_findings;

    #[test]
    fn a_finding_is_read_and_its_trace_is_not() {
        let output = concat!(
            "/p/uaf.c:12:7: warning: Use of memory after it is released [clang-analyzer-unix.Malloc]\n",
            "   12 |         p[0] = 1;\n",
            "      |         ~~~~ ^\n",
            "/p/uaf.c:8:6: note: Memory is allocated\n",
        );
        let findings = parse_findings(output);

        assert_eq!(findings.len(), 1, "a note is part of the trace, not a finding");
        assert_eq!(findings[0].check, "clang-analyzer-unix.Malloc");
        assert_eq!(findings[0].message, "Use of memory after it is released");
        assert_eq!((findings[0].line, findings[0].column), (12, 7));
    }

    #[test]
    fn a_missing_lens_is_an_answer_rather_than_a_failure() {
        use std::path::PathBuf;

        use super::{ClangTidy, ClangTidyConfig, ClangTidyError};

        // The caller treats the lens as optional, so an absent one has to come
        // back as something to discard — never as a panic, and never as a
        // reason the run cannot continue.
        let error = ClangTidy::locate(ClangTidyConfig {
            executable: Some(PathBuf::from("/nonexistent/clang-tidy")),
            ..ClangTidyConfig::default()
        })
        .expect_err("a path that does not exist cannot be verified");

        assert!(matches!(error, ClangTidyError::Unavailable(_)));
    }

    #[test]
    fn source_text_ending_in_a_bracket_is_not_read_as_a_check() {
        // The quoted source the tool echoes back can end in `]`, and reading
        // that as a check name would invent a finding out of the program.
        let output = "   12 |         value = table[index];\n";

        assert!(parse_findings(output).is_empty());
    }
}
