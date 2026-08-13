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
//! Which checker is found is deliberately not this module's business: it
//! resolves `valgrind` on `PATH` and verifies that it answers `--version`, so a
//! platform port satisfies it as readily as the upstream build. That is what
//! makes macOS work through `LouisBrunner/valgrind-macos` without a line of
//! code here, and it is why Windows is answered with WSL rather than with a
//! second tool: a different checker means a different output format, a second
//! version to pin, a second proof in CI, and findings that cannot be compared
//! with these. One oracle per question is the rule the Norminette adapter
//! follows, and a leak checker is not the place to break it.
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

/// One allocation the checker reported as lost.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LeakSite {
    /// Bytes lost through this allocation.
    pub bytes: u64,
    /// Whether these bytes were reachable only through another lost block.
    pub indirect: bool,
    /// The function the allocation was made in.
    pub function: String,
    /// Source file and one-based line, when the binary carries debug
    /// information. A program built without it has no line to name, and that is
    /// worth saying rather than leaving blank.
    pub location: Option<LeakLocation>,
}

/// A source position inside the checked program.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LeakLocation {
    /// File as the checker spelled it.
    pub file: String,
    /// One-based line.
    pub line: u32,
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
    /// Where the lost memory was allocated, in the order the checker listed it.
    pub sites: Vec<LeakSite>,
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
        sites: leak_sites(output),
        output: output.to_owned(),
    })
}

/// Reads the allocation site of every block the checker called lost.
///
/// A loss record is a header naming the bytes and the kind, followed by the
/// stack that allocated them. The frame that matters is the first one inside
/// the program: the top of the stack is always the checker's own replacement
/// allocator, which tells a reader nothing about their code. A binary built
/// without debug information still names its functions, so the site is reported
/// with whatever the checker could resolve rather than dropped.
fn leak_sites(output: &str) -> Vec<LeakSite> {
    let mut sites = Vec::new();
    let mut pending: Option<(u64, bool)> = None;
    for line in output.lines() {
        let line = strip_process_prefix(line);
        if let Some((bytes, indirect)) = loss_record_header(line) {
            pending = Some((bytes, indirect));
            continue;
        }
        let Some((bytes, indirect)) = pending else {
            continue;
        };
        let Some(frame) = stack_frame(line) else {
            // The record ended without a frame this code could read.
            if line.trim().is_empty() {
                pending = None;
            }
            continue;
        };
        if is_checker_internal(&frame.0) {
            continue;
        }
        sites.push(LeakSite {
            bytes,
            indirect,
            function: frame.0,
            location: frame.1,
        });
        pending = None;
    }
    sites
}

/// Removes the `==1234==` the checker puts in front of every line.
fn strip_process_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix("==") else {
        return line;
    };
    rest.split_once("==").map_or(line, |(_, tail)| tail)
}

/// Reads `4,384 bytes in 2 blocks are definitely lost in loss record 5 of 7`.
fn loss_record_header(line: &str) -> Option<(u64, bool)> {
    let trimmed = line.trim();
    let (count, rest) = trimmed.split_once(" bytes in ")?;
    let indirect = if rest.contains("are definitely lost") {
        false
    } else if rest.contains("are indirectly lost") {
        true
    } else {
        return None;
    };
    let digits = count
        .trim()
        .chars()
        .filter(|character| *character != ',')
        .collect::<String>();
    digits.parse().ok().map(|bytes| (bytes, indirect))
}

/// Reads `at 0x4848464: malloc (vg_replace_malloc.c:446)`.
fn stack_frame(line: &str) -> Option<(String, Option<LeakLocation>)> {
    let trimmed = line.trim();
    let rest = trimmed
        .strip_prefix("at ")
        .or_else(|| trimmed.strip_prefix("by "))?;
    let (_, named) = rest.split_once(": ")?;
    let Some((function, tail)) = named.split_once(" (") else {
        return Some((named.trim().to_owned(), None));
    };
    let inside = tail.strip_suffix(')').unwrap_or(tail);
    let location = inside
        .rsplit_once(':')
        .and_then(|(file, line)| Some((file, line.parse::<u32>().ok()?)))
        .map(|(file, line)| LeakLocation {
            file: file.to_owned(),
            line,
        });
    Some((function.trim().to_owned(), location))
}

/// Whether a frame belongs to the checker rather than to the program.
fn is_checker_internal(function: &str) -> bool {
    const REPLACED: [&str; 6] = [
        "malloc",
        "calloc",
        "realloc",
        "operator new",
        "operator new[]",
        "memalign",
    ];
    REPLACED.contains(&function)
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

    /// Loss records as memcheck prints them, including the replacement
    /// allocator frame that must never be reported as the reader's own code.
    const WITH_SITES: &str = "\
==12345== 4,384 bytes in 2 blocks are definitely lost in loss record 5 of 7
==12345==    at 0x4848464: malloc (vg_replace_malloc.c:446)
==12345==    by 0x1091AB: create_stack (stack.c:23)
==12345==    by 0x109245: main (main.c:12)
==12345==
==12345== 48 bytes in 1 blocks are indirectly lost in loss record 2 of 7
==12345==    at 0x4848464: malloc (vg_replace_malloc.c:446)
==12345==    by 0x1092F0: push_node (node.c:41)
==12345==
==12345== LEAK SUMMARY:
==12345==    definitely lost: 4,384 bytes in 2 blocks
==12345==    indirectly lost: 48 bytes in 1 blocks
==12345==
==12345== ERROR SUMMARY: 8 errors from 8 contexts (suppressed: 0 from 0)
";

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
    fn every_lost_block_names_where_it_was_allocated() {
        let report = super::parse_report(WITH_SITES, Some(1)).expect("a readable report");

        assert_eq!(report.sites.len(), 2, "{:?}", report.sites);
        let first = &report.sites[0];
        assert_eq!(first.bytes, 4384);
        assert!(!first.indirect);
        assert_eq!(first.function, "create_stack");
        let location = first.location.as_ref().expect("a source location");
        assert_eq!(location.file, "stack.c");
        assert_eq!(location.line, 23);

        let second = &report.sites[1];
        assert_eq!(second.bytes, 48);
        assert!(second.indirect);
        assert_eq!(second.function, "push_node");
    }

    #[test]
    fn the_checkers_own_allocator_is_never_reported_as_the_readers_code() {
        // `malloc` at the top of every trace is memcheck's replacement, and
        // naming it would point every leak at the same useless line.
        let report = super::parse_report(WITH_SITES, Some(1)).expect("a readable report");
        assert!(
            report.sites.iter().all(|site| site.function != "malloc"),
            "{:?}",
            report.sites
        );
    }

    #[test]
    fn a_binary_without_debug_information_still_names_its_function() {
        let output = concat!(
            "==1== 32 bytes in 1 blocks are definitely lost in loss record 1 of 1\n",
            "==1==    at 0x4848464: malloc (vg_replace_malloc.c:446)\n",
            "==1==    by 0x109245: build_table\n",
            "==1==\n",
            "==1== LEAK SUMMARY:\n",
            "==1==    definitely lost: 32 bytes in 1 blocks\n",
            "==1== ERROR SUMMARY: 1 errors from 1 contexts (suppressed: 0 from 0)\n",
        );
        let report = super::parse_report(output, Some(0)).expect("a readable report");
        let site = report.sites.first().expect("one site");
        assert_eq!(site.function, "build_table");
        assert!(site.location.is_none());
    }

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
