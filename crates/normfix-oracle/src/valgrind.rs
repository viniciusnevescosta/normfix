//! Bounded adapter for the Valgrind memcheck leak checker.
//!
//! This is the only adapter that runs a program the student wrote. Everything
//! else in normfix reads source, and reading source cannot delete a file, open
//! a socket, or spawn anything. Executing a binary can do all three, so the
//! boundary is drawn as narrowly as the feature allows: normfix runs a binary
//! it is pointed at, and never builds one. Building means running Make recipes,
//! which is a second and much larger category of arbitrary execution, and "you
//! built it, I ran it" is a far smaller promise than "I built and ran it".
//!
//! What comes back is evidence, not a verdict. Valgrind reports what it
//! observed on the one path the program took with the arguments it was given;
//! a clean report is not a proof that the program never leaks. The one thing
//! this module must never do is turn silence into that proof, so output it
//! cannot parse is an operational failure rather than an absence of findings.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::executable::resolve_executable;
use crate::norminette::strip_terminal_sequences;
use crate::process::{ProcessError, ProcessLimits, run_bounded};

/// Configuration for the optional leak checker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValgrindConfig {
    /// Explicit `valgrind`, or `None` to search `PATH`.
    pub executable: Option<PathBuf>,
    /// Wall-clock and output limits for the checked run.
    pub limits: ProcessLimits,
}

impl Default for ValgrindConfig {
    fn default() -> Self {
        Self {
            executable: None,
            // A program under memcheck runs tens of times slower than normally,
            // so this is far more generous than the compiler's ten seconds. It
            // is still a bound: a program that waits for input it will never
            // get has to stop being normfix's problem at some point.
            limits: ProcessLimits {
                timeout: std::time::Duration::from_secs(120),
                output_bytes: 4 * 1024 * 1024,
            },
        }
    }
}

/// What one leak-checked run observed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValgrindReport {
    /// Exit status of the program itself, when it exited normally.
    pub program_exit_code: Option<i32>,
    /// Bytes still reachable through no pointer at exit.
    pub definitely_lost_bytes: u64,
    /// Bytes reachable only through a block that was itself lost.
    pub indirectly_lost_bytes: u64,
    /// Bytes still allocated at exit, reachable through a pointer.
    pub still_reachable_bytes: u64,
    /// Memory errors memcheck reported, which are not only leaks.
    pub error_count: u64,
    /// Bounded checker output, kept so a reader can see what it saw.
    pub output: String,
}

impl ValgrindReport {
    /// Whether anything was lost outright.
    ///
    /// `still_reachable` is deliberately excluded: memory a program still holds
    /// a pointer to at exit is not a leak by the definition 42 evaluates, and
    /// counting it would flood a correct project with findings.
    #[must_use]
    pub const fn lost_anything(&self) -> bool {
        self.definitely_lost_bytes > 0 || self.indirectly_lost_bytes > 0
    }
}

/// Operational failure, distinct from anything the checker found.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ValgrindError {
    /// The checker is not installed, or not on this platform.
    #[error("Valgrind is unavailable: {0}")]
    Unavailable(String),
    /// The program to check is not a file this process may execute.
    #[error("cannot run `{path}` under Valgrind: {detail}")]
    UnusableProgram {
        /// Program as requested.
        path: PathBuf,
        /// Why it was refused.
        detail: String,
    },
    /// Version verification failed.
    #[error("Valgrind version check failed (exit {exit_code:?}): {detail}")]
    VersionFailure {
        /// Numeric status when available.
        exit_code: Option<i32>,
        /// Bounded detail.
        detail: String,
    },
    /// The checker ran but its report could not be read.
    ///
    /// This exists so that unreadable output can never be mistaken for a clean
    /// result. A leak checker that says nothing and a leak checker that found
    /// nothing look identical until this distinction is made explicit.
    #[error("Valgrind output could not be read as a leak summary: {0}")]
    UnreadableReport(String),
    /// The bounded child-process runner failed.
    #[error(transparent)]
    Process(#[from] ProcessError),
}

/// A located, version-verified leak checker.
#[derive(Clone, Debug)]
pub struct ValgrindChecker {
    executable: PathBuf,
    limits: ProcessLimits,
}

impl ValgrindChecker {
    /// Locates the checker and confirms it answers `--version`.
    ///
    /// # Errors
    ///
    /// Returns [`ValgrindError::Unavailable`] when no usable checker exists,
    /// which is the normal outcome on a platform Valgrind does not support.
    pub fn locate(config: ValgrindConfig) -> Result<Self, ValgrindError> {
        let ValgrindConfig { executable, limits } = config;
        let executable = resolve_executable(executable.as_deref(), "valgrind")
            .map_err(ValgrindError::Unavailable)?;
        let checker = Self { executable, limits };
        checker.verify_version()?;
        Ok(checker)
    }

    /// Path of the located checker.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    fn verify_version(&self) -> Result<(), ValgrindError> {
        let mut command = Command::new(&self.executable);
        command.arg("--version");
        let output = run_bounded(&mut command, self.limits)?;
        if output.exit_code != Some(0) {
            return Err(ValgrindError::VersionFailure {
                exit_code: output.exit_code,
                detail: strip_terminal_sequences(&output.stderr),
            });
        }
        Ok(())
    }

    /// Runs one program under memcheck and reports what it observed.
    ///
    /// `arguments` are passed to the program, not to Valgrind, so a project
    /// that needs input can be exercised on the path that matters.
    ///
    /// # Errors
    ///
    /// Returns [`ValgrindError`] when the program cannot be run, or when the
    /// checker produced output this adapter cannot read as a leak summary.
    pub fn check(
        &self,
        program: &Path,
        arguments: &[String],
    ) -> Result<ValgrindReport, ValgrindError> {
        let program =
            resolve_executable(Some(program), "the program to check").map_err(|detail| {
                ValgrindError::UnusableProgram {
                    path: program.to_path_buf(),
                    detail,
                }
            })?;

        let mut command = Command::new(&self.executable);
        command
            .arg("--leak-check=full")
            .arg("--show-leak-kinds=definite,indirect")
            // Without this the summary reports the count but not the bytes for
            // indirect losses, and the report would understate what was lost.
            .arg("--errors-for-leak-kinds=definite,indirect")
            .arg("--error-exitcode=0")
            .arg(&program)
            .args(arguments);
        let output = run_bounded(&mut command, self.limits)?;

        let combined = format!(
            "{}{}",
            strip_terminal_sequences(&output.stdout),
            strip_terminal_sequences(&output.stderr)
        );
        parse_report(&combined, output.exit_code)
    }
}

/// Reads a memcheck summary.
///
/// Split out from the run so the parsing can be tested against real checker
/// output without a checker, and so the failure to parse is visibly its own
/// outcome rather than a zero.
fn parse_report(
    output: &str,
    program_exit_code: Option<i32>,
) -> Result<ValgrindReport, ValgrindError> {
    let definitely_lost_bytes = leak_bytes(output, "definitely lost");
    let indirectly_lost_bytes = leak_bytes(output, "indirectly lost");
    let still_reachable_bytes = leak_bytes(output, "still reachable");
    let error_count = error_summary_count(output);

    // A run that produced no summary at all is not a clean run. It is a run
    // whose result is unknown, and saying so is the whole point of this branch.
    if definitely_lost_bytes.is_none() && error_count.is_none() {
        return Err(ValgrindError::UnreadableReport(bounded_detail(output)));
    }

    Ok(ValgrindReport {
        program_exit_code,
        definitely_lost_bytes: definitely_lost_bytes.unwrap_or(0),
        indirectly_lost_bytes: indirectly_lost_bytes.unwrap_or(0),
        still_reachable_bytes: still_reachable_bytes.unwrap_or(0),
        error_count: error_count.unwrap_or(0),
        output: output.to_owned(),
    })
}

/// Reads one `LEAK SUMMARY` line, such as `definitely lost: 1,024 bytes in 2 blocks`.
fn leak_bytes(output: &str, label: &str) -> Option<u64> {
    for line in output.lines() {
        let Some(rest) = line.split_once(label) else {
            continue;
        };
        let digits = rest
            .1
            .trim_start_matches(|c: char| c == ':' || c.is_whitespace())
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == ',')
            .filter(|c| *c != ',')
            .collect::<String>();
        if !digits.is_empty() {
            return digits.parse().ok();
        }
    }
    None
}

/// Reads the `ERROR SUMMARY: N errors` line.
fn error_summary_count(output: &str) -> Option<u64> {
    for line in output.lines() {
        let Some((_, rest)) = line.split_once("ERROR SUMMARY:") else {
            continue;
        };
        let digits = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == ',')
            .filter(|c| *c != ',')
            .collect::<String>();
        if !digits.is_empty() {
            return digits.parse().ok();
        }
    }
    None
}

/// Keeps an unreadable-output error informative without pasting a megabyte.
fn bounded_detail(output: &str) -> String {
    const LIMIT: usize = 400;
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return "the checker produced no output".to_owned();
    }
    match trimmed.char_indices().nth(LIMIT) {
        Some((index, _)) => format!("{}…", &trimmed[..index]),
        None => trimmed.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ValgrindError, parse_report};

    /// A real memcheck summary from a program that lost a block.
    const LEAKING: &str = "\
==12345== HEAP SUMMARY:
==12345==     in use at exit: 1,124 bytes in 3 blocks
==12345==   total heap usage: 4 allocs, 1 frees, 2,148 bytes allocated
==12345==
==12345== LEAK SUMMARY:
==12345==    definitely lost: 1,024 bytes in 2 blocks
==12345==    indirectly lost: 100 bytes in 1 blocks
==12345==      possibly lost: 0 bytes in 0 blocks
==12345==    still reachable: 0 bytes in 0 blocks
==12345==         suppressed: 0 bytes in 0 blocks
==12345==
==12345== ERROR SUMMARY: 2 errors from 2 contexts (suppressed: 0 from 0)
";

    /// A program that freed everything, which is the common case.
    const CLEAN: &str = "\
==12345== HEAP SUMMARY:
==12345==     in use at exit: 0 bytes in 0 blocks
==12345== All heap blocks were freed -- no leaks are possible
==12345==
==12345== ERROR SUMMARY: 0 errors from 0 contexts (suppressed: 0 from 0)
";

    #[test]
    fn a_leak_summary_is_read_with_its_thousands_separators() {
        let report = parse_report(LEAKING, Some(0)).expect("a real summary");

        assert_eq!(report.definitely_lost_bytes, 1024);
        assert_eq!(report.indirectly_lost_bytes, 100);
        assert_eq!(report.error_count, 2);
        assert!(report.lost_anything());
    }

    #[test]
    fn a_program_that_freed_everything_reports_nothing_lost() {
        // Valgrind omits the leak summary entirely in this case, so the error
        // summary is the only thing proving the checker actually ran.
        let report = parse_report(CLEAN, Some(0)).expect("a clean run is still a run");

        assert_eq!(report.definitely_lost_bytes, 0);
        assert_eq!(report.error_count, 0);
        assert!(!report.lost_anything());
    }

    #[test]
    fn output_that_cannot_be_read_is_never_reported_as_clean() {
        // The failure this whole module is shaped around. A checker that was
        // killed, or that changed its output format, must not be
        // indistinguishable from a checker that found nothing.
        for output in ["", "valgrind: cannot execute", "==1== Command: ./program"] {
            let error = parse_report(output, Some(0))
                .expect_err("unreadable output must not become a clean result");
            assert!(
                matches!(error, ValgrindError::UnreadableReport(_)),
                "{output:?}"
            );
        }
    }

    #[test]
    fn memory_still_held_at_exit_is_not_counted_as_lost() {
        // 42 evaluates memory nobody can reach any more. A program that keeps a
        // pointer to its arena until exit is not leaking by that definition, and
        // counting it would bury real findings under correct programs.
        let held = "\
==1== LEAK SUMMARY:
==1==    definitely lost: 0 bytes in 0 blocks
==1==    indirectly lost: 0 bytes in 0 blocks
==1==    still reachable: 8,192 bytes in 4 blocks
==1== ERROR SUMMARY: 0 errors from 0 contexts (suppressed: 0 from 0)
";
        let report = parse_report(held, Some(0)).expect("a real summary");

        assert_eq!(report.still_reachable_bytes, 8192);
        assert!(!report.lost_anything());
    }

    /// Drives the real checker against a program that really leaks.
    ///
    /// Ignored by default because Valgrind is not installed everywhere and does
    /// not exist on every platform normfix supports. CI runs it where it does,
    /// which is the only place the claim can be made.
    #[test]
    #[ignore = "requires an installed Valgrind and a C compiler"]
    fn installed_valgrind_smoke_test() {
        use std::process::Command;

        use tempfile::TempDir;

        use super::{ValgrindChecker, ValgrindConfig};

        let directory = TempDir::new().expect("temporary directory");
        let source = directory.path().join("leak.c");
        std::fs::write(
            &source,
            "#include <stdlib.h>\nint main(void){char *p = malloc(1024); (void)p; return 0;}\n",
        )
        .expect("write a leaking program");
        let program = directory.path().join("leak");
        let compiled = Command::new(std::env::var("NORMFIX_TEST_CC").as_deref().unwrap_or("cc"))
            .arg("-g")
            .arg("-o")
            .arg(&program)
            .arg(&source)
            .status()
            .expect("run the compiler");
        assert!(compiled.success(), "the test program must compile");

        let checker = ValgrindChecker::locate(ValgrindConfig::default())
            .expect("this test is only run where Valgrind is installed");
        let report = checker
            .check(&program, &[])
            .expect("a real leak-checked run");

        assert_eq!(
            report.definitely_lost_bytes, 1024,
            "the checker did not see the leak this program was written to have: {}",
            report.output,
        );
        assert!(report.lost_anything());
    }

    #[test]
    fn the_program_exit_status_is_carried_through() {
        // The checker is told not to change it, so a non-zero status here is
        // the program's own and worth reporting beside the leak result.
        let report = parse_report(CLEAN, Some(3)).expect("a clean run");

        assert_eq!(report.program_exit_code, Some(3));
    }
}
